//! DS-6.1 — archetype-driven dashboard layout.
//!
//! The dashboard used to tile its widgets uniformly: pick a column count from
//! the widget count and the available width, then give every widget the same
//! box. That is one layout, and it is nobody's design — Aperture's mosaic and
//! the Lucid/Meridien editorial grid are both explicitly non-uniform.
//!
//! This module turns [`Archetype`] into real geometry on the M4.4 [`Grid`]:
//!
//! | Archetype      | Grid                                          |
//! |----------------|-----------------------------------------------|
//! | `Mosaic`       | 12 cols x 92px auto-rows, gap 12, typed spans |
//! | `Editorial`    | 300px / 1fr / 360px, rows 1.1 / 1.0 / 0.9     |
//! | `DenseScreens` | uniform, tighter gap                          |
//! | `TradingShell` | uniform — today's behaviour, unchanged        |
//!
//! ## Why this is a workspace view and not a shell mode
//!
//! DS-6.0 decision D3. A shell mode would restructure the root panel stack,
//! which lives in `core.rs` — sacred, and excluded from every design-system
//! sweep. The dashboard is already a pane the workspace system owns, so
//! archetype changes what that pane draws and touches nothing below it.
//!
//! ## Why the widget's KIND picks its span
//!
//! `06-LAYOUT-ARCHETYPES` §2 gives a shape table (1x1 index pill, 4x2 hero
//! P&L, 6x2 watchlist/news, 12x1 action strip). Those shapes are semantic:
//! a news ticker is wide because headlines are a line of text, and a gauge is
//! square because it draws a ring. So the span comes from what the widget IS,
//! not from its position in the list — which also means the mosaic keeps its
//! character no matter how many widgets are visible or what order they are in.

use egui::{Rect, Vec2};

use crate::chart_renderer::ChartWidgetKind as K;
use crate::design_system::style_system::Archetype;
use crate::ui_kit::layout::grid::{Grid, GridItem, Track};

/// Aperture's mosaic row height (`grid-auto-rows: 92px`).
const MOSAIC_ROW_H: f32 = 92.0;
/// Aperture's tile gap (`gap: 12px` — tiles use 12, pages use 10).
const MOSAIC_GAP: f32 = 12.0;
/// The editorial grid's fixed rails (`300px 1fr 360px`).
const EDITORIAL_LEFT: f32 = 300.0;
const EDITORIAL_RIGHT: f32 = 360.0;
/// Below this the fixed rails stop making sense and the grid falls back to
/// uniform tiling. `06` §8 records 980px as the mosaic's stated `min-width`;
/// the editorial rails need 300 + 360 + a workable centre.
const EDITORIAL_MIN_W: f32 = 820.0;
const MOSAIC_MIN_W: f32 = 980.0;

/// Column/row span for a widget in Aperture's 12-column mosaic.
///
/// Shapes are from the `06-LAYOUT-ARCHETYPES` §2 table. Anything unlisted gets
/// 2x2 — the table's "KPI, ring" default, and the shape most widgets here are.
pub fn mosaic_span(kind: K) -> (u16, u16) {
    match kind {
        // 12x1 — full-width strip. A headline crawl is a line of text.
        K::NewsTicker => (12, 1),

        // 6x2 — the wide reading surfaces.
        K::SignalDashboard | K::SignalRadar | K::CrossAssetPulse | K::MarketBreadth => (6, 2),

        // 4x2 — hero.
        K::PositionPnl => (4, 2),

        // 3x2 — chart / ladder shaped: a horizontal axis to read along.
        K::VolumeProfile | K::VolumeShelf | K::KeyLevels | K::SectorRotation
        | K::MomentumHeat | K::ChangePoints | K::TradePlan => (3, 2),

        // 2x1 — small chips with a single number or countdown.
        K::SessionTimer | K::EarningsBadge => (2, 1),

        // 2x2 — gauges and rings (the default).
        _ => (2, 2),
    }
}

/// Column span for a widget in the editorial 3-rail grid.
///
/// The rails are semantic (left = lists, centre = the main read, right =
/// news/summary), so only the genuinely full-bleed widgets span them.
fn editorial_span(kind: K) -> u16 {
    match kind {
        K::NewsTicker => 3,
        K::SignalDashboard | K::CrossAssetPulse => 2,
        _ => 1,
    }
}

/// Solve tile rectangles for `kinds` inside `body`, per `archetype`.
///
/// Returns one rect per input kind, in order. An empty input, a zero-size
/// body, or a body too narrow for the archetype's fixed rails all fall back to
/// uniform tiling rather than producing degenerate boxes.
pub fn solve(archetype: Archetype, body: Rect, kinds: &[K], uniform_gap: f32) -> Vec<Rect> {
    if kinds.is_empty() || body.width() <= 0.0 || body.height() <= 0.0 {
        return Vec::new();
    }
    let avail = Vec2::new(body.width(), body.height());

    let rects = match archetype {
        Archetype::Mosaic if body.width() >= MOSAIC_MIN_W => {
            let mut g = Grid::new()
                .cols(Track::fr_repeat(12, 1.0))
                .auto_rows(MOSAIC_ROW_H)
                .gap(MOSAIC_GAP);
            for k in kinds {
                let (c, r) = mosaic_span(*k);
                g = g.item(GridItem::new().col_span(c).row_span(r));
            }
            g.solve(avail)
        }

        Archetype::Editorial if body.width() >= EDITORIAL_MIN_W => {
            // Fixed rails; the centre takes what is left. Rows carry the
            // 1.1 / 1.0 / 0.9 rhythm — the top `auto` row in the CSS is the
            // pane header, which the caller has already reserved.
            let mut g = Grid::new()
                .cols(vec![
                    Track::px(EDITORIAL_LEFT),
                    Track::fr(1.0),
                    Track::px(EDITORIAL_RIGHT),
                ])
                .rows(vec![Track::fr(1.1), Track::fr(1.0), Track::fr(0.9)])
                // Beyond the three authored rows the grid keeps going at the
                // middle row's share, so a widget-heavy workspace scrolls
                // instead of crushing the rhythm.
                .auto_rows((avail.y / 3.0).max(120.0))
                .gap(uniform_gap * 2.0);
            for k in kinds {
                g = g.item(GridItem::new().col_span(editorial_span(*k)));
            }
            g.solve(avail)
        }

        // DenseScreens keeps the uniform tiling but tightens the gutter — its
        // character is "more on screen", not a different skeleton.
        Archetype::DenseScreens => return uniform(body, kinds.len(), uniform_gap * 0.5),

        // TradingShell (and any archetype whose rails do not fit) — unchanged.
        _ => return uniform(body, kinds.len(), uniform_gap),
    };

    // Grid solves in local space; shift into the body's coordinates.
    let offset = body.min.to_vec2();
    rects.into_iter().map(|r| r.translate(offset)).collect()
}

/// The pre-DS-6.1 layout: pick a column count from width and count, then give
/// every tile the same box. Still correct for the trading shell, and the
/// fallback whenever a fixed-rail archetype has too little width to honour.
pub fn uniform(body: Rect, n: usize, gap: f32) -> Vec<Rect> {
    if n == 0 || body.width() <= 0.0 || body.height() <= 0.0 {
        return Vec::new();
    }
    let content = Rect::from_min_max(
        egui::pos2(body.left() + gap, body.top() + gap),
        egui::pos2(body.right() - gap, body.bottom() - gap),
    );
    let avail_w = content.width();
    let cols = if avail_w > 600.0 && n >= 6 { 4 }
        else if avail_w > 450.0 && n >= 4 { 3 }
        else if avail_w > 250.0 && n >= 2 { 2 }
        else { 1 };
    let rows = n.div_ceil(cols);
    let tile_w = (avail_w - (cols - 1) as f32 * gap) / cols as f32;
    let tile_h = ((content.height() - (rows - 1) as f32 * gap) / rows as f32).clamp(60.0, 280.0);

    (0..n)
        .map(|i| {
            let (col, row) = (i % cols, i / cols);
            Rect::from_min_size(
                egui::pos2(
                    content.left() + col as f32 * (tile_w + gap),
                    content.top() + row as f32 * (tile_h + gap),
                ),
                Vec2::new(tile_w, tile_h),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(w: f32, h: f32) -> Rect {
        Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(w, h))
    }

    /// Aperture's mosaic must solve to the EXACT geometry `06` specifies:
    /// 12 columns, 92px rows, 12px gap. This is the same arithmetic the M4.4
    /// Grid work proved (4-col hero = 436px, 2-row = 196px) — re-asserted here
    /// through the archetype entry point so the two cannot drift.
    #[test]
    fn mosaic_matches_the_specified_geometry() {
        // A 12-col grid of 1fr at width W with 11 gaps: col = (W - 11*12) / 12.
        // Pick W so columns land on a whole number: 12*32 + 11*12 = 516.
        let r = solve(Archetype::Mosaic, body(1000.0, 600.0), &[K::PositionPnl], 6.0);
        assert_eq!(r.len(), 1);
        let col_w = (1000.0 - 11.0 * MOSAIC_GAP) / 12.0;
        // Hero P&L is 4x2: four columns + the three gaps between them.
        let want_w = 4.0 * col_w + 3.0 * MOSAIC_GAP;
        let want_h = 2.0 * MOSAIC_ROW_H + MOSAIC_GAP;
        assert!((r[0].width() - want_w).abs() < 0.5, "hero width {} != {want_w}", r[0].width());
        assert!((r[0].height() - want_h).abs() < 0.5, "hero height {} != {want_h}", r[0].height());
    }

    /// The news ticker spans all 12 columns — the full-width action strip.
    #[test]
    fn mosaic_news_is_full_bleed() {
        let r = solve(Archetype::Mosaic, body(1200.0, 600.0), &[K::NewsTicker], 6.0);
        assert!(
            (r[0].width() - 1200.0).abs() < 0.5,
            "12-col span must fill the body, got {}", r[0].width()
        );
        assert!((r[0].height() - MOSAIC_ROW_H).abs() < 0.5, "12x1 is one row tall");
    }

    /// The editorial rails are FIXED — 300 left, 360 right, centre takes the
    /// rest. That is the whole identity of the Lucid/Meridien dashboard.
    #[test]
    fn editorial_rails_are_fixed_width() {
        let w = 1400.0;
        let gap = 6.0;
        let r = solve(Archetype::Editorial, body(w, 700.0),
                      &[K::MarketBreadth, K::TrendStrength, K::Correlation], gap);
        assert_eq!(r.len(), 3);
        assert!((r[0].width() - EDITORIAL_LEFT).abs() < 0.5,  "left rail 300, got {}", r[0].width());
        assert!((r[2].width() - EDITORIAL_RIGHT).abs() < 0.5, "right rail 360, got {}", r[2].width());
        // Centre absorbs the remainder, so widening the body must widen ONLY it.
        let r2 = solve(Archetype::Editorial, body(w + 400.0, 700.0),
                       &[K::MarketBreadth, K::TrendStrength, K::Correlation], gap);
        assert!((r2[0].width() - r[0].width()).abs() < 0.5, "left rail must not move");
        assert!((r2[2].width() - r[2].width()).abs() < 0.5, "right rail must not move");
        assert!(r2[1].width() > r[1].width() + 300.0, "centre must absorb the extra width");
    }

    /// The editorial rows carry a 1.1 / 1.0 / 0.9 rhythm — descending, not
    /// equal. Equal rows would silently turn the editorial grid into a plain
    /// 3x3, which is exactly the uniform layout it exists to replace.
    #[test]
    fn editorial_rows_descend() {
        let kinds = [K::TrendStrength; 9];
        let r = solve(Archetype::Editorial, body(1400.0, 900.0), &kinds, 6.0);
        assert_eq!(r.len(), 9);
        let (r1, r2, r3) = (r[0].height(), r[3].height(), r[6].height());
        assert!(r1 > r2 && r2 > r3, "rows must descend 1.1 > 1.0 > 0.9, got {r1} {r2} {r3}");
    }

    /// Narrow bodies fall back to uniform rather than emitting rails wider than
    /// the pane. A 300px rail in a 400px pane is not a design, it is a bug.
    #[test]
    fn narrow_body_falls_back_to_uniform() {
        let kinds = [K::TrendStrength, K::Momentum];
        for arch in [Archetype::Editorial, Archetype::Mosaic] {
            let r = solve(arch, body(500.0, 400.0), &kinds, 6.0);
            assert_eq!(r.len(), 2);
            for rect in &r {
                assert!(rect.width() <= 500.0, "{arch:?} tile {} exceeds a 500px body", rect.width());
            }
        }
    }

    /// Every archetype returns exactly one rect per widget, positioned inside
    /// the body's coordinate space (the Grid solves at the origin, so a missing
    /// translate would silently stack every dashboard at the window corner).
    #[test]
    fn all_archetypes_return_one_rect_per_widget_in_body_space() {
        let kinds = [K::TrendStrength, K::NewsTicker, K::PositionPnl, K::SessionTimer];
        for arch in [Archetype::TradingShell, Archetype::DenseScreens,
                     Archetype::Mosaic, Archetype::Editorial] {
            let b = body(1400.0, 900.0);
            let r = solve(arch, b, &kinds, 6.0);
            assert_eq!(r.len(), kinds.len(), "{arch:?} rect count");
            for rect in &r {
                assert!(rect.min.x >= b.min.x - 0.5, "{arch:?} tile left of body");
                assert!(rect.min.y >= b.min.y - 0.5, "{arch:?} tile above body");
            }
        }
    }

    /// The WIRING, not just the geometry: the expression the dashboard pane
    /// evaluates must resolve each theme to its designed archetype.
    ///
    /// The pane does `active_style_system().shell.resolve_archetype(override)`.
    /// Every piece of that is tested elsewhere — the builtins' archetypes, the
    /// precedence rule, the grid maths — but nothing asserted the CHAIN. This
    /// is the same class of gap that let the recipe layer sit fully built and
    /// fully dormant for a milestone: each part correct, never connected.
    #[test]
    fn active_style_resolves_to_its_designed_archetype() {
        use crate::chart_renderer::ui::style::{
            list_style_presets, set_active_style, active_style_system,
            M1_GLOBAL_STATE_TEST_LOCK,
        };
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let presets = list_style_presets();
        let find = |name: &str| presets.iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(name))
            .map(|(i, _)| *i);

        for (style, want) in [
            ("aperture", Archetype::Mosaic),
            ("meridien", Archetype::Editorial),
            ("lucid",    Archetype::Editorial),
            ("cadence",  Archetype::DenseScreens),
            ("mariner",  Archetype::TradingShell),
        ] {
            let Some(idx) = find(style) else { continue };
            set_active_style(idx);
            let got = active_style_system().shell.resolve_archetype(None);
            assert_eq!(got, want, "style '{style}' must resolve to {want:?}");
            // And a workspace override must beat it, through the same call.
            let forced = active_style_system().shell.resolve_archetype(Some(Archetype::Mosaic));
            assert_eq!(forced, Archetype::Mosaic, "override must win for '{style}'");
        }
        set_active_style(0);
    }

    /// Span depends on WHAT the widget is, not where it sits in the list —
    /// so reordering widgets cannot change the mosaic's character.
    #[test]
    fn span_follows_kind_not_position() {
        let a = solve(Archetype::Mosaic, body(1200.0, 800.0), &[K::NewsTicker, K::TrendStrength], 6.0);
        let b = solve(Archetype::Mosaic, body(1200.0, 800.0), &[K::TrendStrength, K::NewsTicker], 6.0);
        // The news ticker is full-bleed in both orders.
        assert!((a[0].width() - 1200.0).abs() < 0.5);
        assert!((b[1].width() - 1200.0).abs() < 0.5);
        // And the gauge is narrow in both.
        assert!(a[1].width() < 300.0 && b[0].width() < 300.0);
    }
}
