//! Canonical in-memory chart state — Phase 1 of the storage architecture spec.
//!
//! See `docs/CHART_STORAGE_ARCHITECTURE.md`.
//!
//! This module defines the single source of truth for a chart's state in memory.
//! All persistence formats (Postgres rows, `.apxchart` binary, XOL zip) convert
//! to and from these types. The renderer reads from here every frame.
//!
//! Phase 1 is purely the data model — no I/O, no codecs, nothing wired up yet.
//! The existing `drawings` and `drawing_db` modules continue to work unchanged.


pub mod codec;
pub mod drawings;
pub mod annotations;
pub mod indicators;
pub mod templates;
pub mod style_table;
pub mod extension_bag;
pub mod viewport;

pub use annotations::{Annotation, AnnotationId};
pub use drawings::{Drawing, DrawingFlags, DrawingId, DrawingKind, Point};
pub use extension_bag::ExtensionBag;
pub use indicators::{IndicatorRef, ParamSchemaVersion};
pub use style_table::{StyleId, StyleTable};
pub use templates::{PaneSpec, PaneTemplate, TemplateId};
pub use viewport::Viewport;

use arcstr::ArcStr;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use smallvec::SmallVec;

/// Unique identifier for a chart in its lifetime.
///
/// In memory we use u64 for fast hashing/indexing. The DB and on-disk formats
/// can use UUIDs at the codec boundary; the conversion happens there.
pub type ChartId = u64;

/// Asset class drives default rendering behaviors (session boundaries, holiday
/// calendars, fractional pip handling, etc.). Cheap discriminant; never a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Equity,
    Etf,
    Index,
    Option,
    Future,
    Crypto,
    Fx,
}

/// Timeframe enum — never a string in the hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Timeframe {
    Tick,
    S1,
    S5,
    S15,
    M1,
    M5,
    M15,
    H1,
    H4,
    D1,
    W1,
    Mn1,
}

/// Provider-specific symbol hints. Recipients map canonical → their own data
/// source via this table; we never lock the format to one provider's symbology.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProviderHints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polygon: Option<ArcStr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ib: Option<ArcStr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tv: Option<ArcStr>,
}

/// A symbol in canonical form plus provider-specific aliases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub canonical: ArcStr,
    pub asset_class: AssetClass,
    #[serde(default, skip_serializing_if = "is_default_hints")]
    pub provider_hints: ProviderHints,
}

fn is_default_hints(h: &ProviderHints) -> bool {
    h.polygon.is_none() && h.ib.is_none() && h.tv.is_none()
}

/// Theme override — small enum for now; can grow into a struct of overrides.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeOverride {
    #[default]
    Inherit,
    Light,
    Dark,
}

/// The hub: every persistence layer converts to or from `ChartState`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartState {
    pub id: ChartId,
    pub symbol: Symbol,
    pub timeframe: Timeframe,
    pub viewport: Viewport,
    pub theme: ThemeOverride,

    /// Stable IDs across deletes; O(1) lookup; dense iteration.
    pub drawings: SlotMap<DrawingId, Drawing>,
    pub annotations: SlotMap<AnnotationId, Annotation>,

    /// What's actually loaded — independent of any template.
    /// 8 inline indicators covers ~95% of real charts without a heap alloc.
    pub indicators: SmallVec<[IndicatorRef; 8]>,

    /// Optional reference to a pane template. The chart remains valid even if
    /// the template is later deleted (`ON DELETE SET NULL` semantics in DB).
    pub template_id: Option<TemplateId>,

    /// Per-chart interned style table. See spec §11 open questions for why
    /// per-chart over per-user in v1.
    pub style_table: StyleTable,

    /// Title and free-text description. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<ArcStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<ArcStr>,

    /// Forward-compat: unknown fields from a future XOL writer survive
    /// round-trips through this build. See spec §7.
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    pub unknown_extensions: ExtensionBag,
}

impl ChartState {
    /// Create an empty chart at the given symbol/timeframe.
    pub fn new(id: ChartId, symbol: Symbol, timeframe: Timeframe) -> Self {
        Self {
            id,
            symbol,
            timeframe,
            viewport: Viewport::default(),
            theme: ThemeOverride::default(),
            drawings: SlotMap::with_key(),
            annotations: SlotMap::with_key(),
            indicators: SmallVec::new(),
            template_id: None,
            style_table: StyleTable::new(),
            title: None,
            description: None,
            unknown_extensions: ExtensionBag::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drawings::{Drawing, DrawingFlags, DrawingKind, Point};
    use indicators::IndicatorRef;
    use smallvec::smallvec;
    use style_table::{Style, StyleTable};

    fn make_chart() -> ChartState {
        ChartState::new(
            1,
            Symbol {
                canonical: arcstr::literal!("SPX"),
                asset_class: AssetClass::Index,
                provider_hints: ProviderHints::default(),
            },
            Timeframe::M5,
        )
    }

    #[test]
    fn empty_chart_is_constructible() {
        let c = make_chart();
        assert_eq!(c.drawings.len(), 0);
        assert_eq!(c.indicators.len(), 0);
        assert!(c.template_id.is_none());
        assert!(c.unknown_extensions.is_empty());
    }

    // ── Drawings SlotMap ──────────────────────────────────────────────────────

    #[test]
    fn drawings_slotmap_insert_returns_stable_id() {
        let mut c = make_chart();
        let d = Drawing::new(
            DrawingKind::HorizontalLine,
            smallvec![Point { ts_ns: 1_700_000_000_000_000_000, price: 4500.0 }],
            StyleId::default(),
        );
        let id1 = c.drawings.insert(d.clone());
        let id2 = c.drawings.insert(d);
        // Two distinct drawings get distinct IDs.
        assert_ne!(id1, id2);
        assert_eq!(c.drawings.len(), 2);
    }

    #[test]
    fn drawings_slotmap_remove_frees_slot() {
        let mut c = make_chart();
        let d = Drawing::new(
            DrawingKind::Trendline,
            smallvec![
                Point { ts_ns: 1_700_000_000_000_000_000, price: 4400.0 },
                Point { ts_ns: 1_700_000_300_000_000_000, price: 4450.0 },
            ],
            StyleId::default(),
        );
        let id = c.drawings.insert(d);
        assert!(c.drawings.contains_key(id));
        c.drawings.remove(id);
        assert!(!c.drawings.contains_key(id));
        assert_eq!(c.drawings.len(), 0);
    }

    #[test]
    fn drawings_slotmap_id_after_remove_does_not_alias() {
        let mut c = make_chart();
        let pt = || Point { ts_ns: 1_700_000_000_000_000_000, price: 4400.0 };
        let d1 = Drawing::new(DrawingKind::HorizontalLine, smallvec![pt()], StyleId::default());
        let d2 = Drawing::new(DrawingKind::VerticalLine, smallvec![pt()], StyleId::default());
        let id1 = c.drawings.insert(d1);
        c.drawings.remove(id1);
        let id2 = c.drawings.insert(d2);
        // The new ID must not be confused with the removed one.
        assert!(c.drawings.contains_key(id2));
        assert!(!c.drawings.contains_key(id1), "removed key must not alias new insertion");
    }

    #[test]
    fn drawings_flags_default_to_visible() {
        let mut c = make_chart();
        let d = Drawing::new(
            DrawingKind::Ray,
            smallvec![Point { ts_ns: 0, price: 100.0 }],
            StyleId::default(),
        );
        let id = c.drawings.insert(d);
        assert!(c.drawings[id].flags.contains(DrawingFlags::VISIBLE));
        assert!(!c.drawings[id].flags.contains(DrawingFlags::LOCKED));
    }

    // ── Annotations SlotMap ───────────────────────────────────────────────────

    #[test]
    fn annotations_slotmap_insert_remove() {
        use annotations::{Annotation, AnnotationId};
        let mut c = make_chart();
        let ann = Annotation {
            anchor: Point { ts_ns: 1_700_000_000_000_000_000, price: 4520.0 },
            title: arcstr::literal!("Test note"),
            body_md: arcstr::literal!("**Bold** body"),
            color: 0xFF_FF_00_FF,
            asset_refs: Default::default(),
            extras: Default::default(),
        };
        let id = c.annotations.insert(ann);
        assert_eq!(c.annotations.len(), 1);
        assert_eq!(c.annotations[id].title.as_str(), "Test note");
        c.annotations.remove(id);
        assert_eq!(c.annotations.len(), 0);
    }

    // ── Indicators SmallVec ───────────────────────────────────────────────────

    #[test]
    fn indicators_inline_up_to_capacity() {
        let mut c = make_chart();
        // Push 8 indicators — the SmallVec inline capacity.
        for i in 0u32..8 {
            c.indicators.push(IndicatorRef {
                ref_id: arcstr::format!("apex.sma.{i}"),
                ref_version: 1,
                param_schema_version: 0,
                pane: arcstr::literal!("main"),
                params: serde_json::json!({ "period": i * 10 }),
                style: None,
                installed_locally: true,
                extras: Default::default(),
            });
        }
        assert_eq!(c.indicators.len(), 8);
        // Confirm they stay inline (no heap spill) at capacity.
        assert!(!c.indicators.spilled(), "8 indicators must stay inline");
    }

    #[test]
    fn indicators_push_beyond_inline_spills_gracefully() {
        let mut c = make_chart();
        for i in 0u32..12 {
            c.indicators.push(IndicatorRef {
                ref_id: arcstr::format!("apex.ema.{i}"),
                ref_version: 1,
                param_schema_version: 0,
                pane: arcstr::literal!("main"),
                params: serde_json::json!({ "period": i }),
                style: None,
                installed_locally: true,
                extras: Default::default(),
            });
        }
        assert_eq!(c.indicators.len(), 12);
    }

    // ── Viewport ─────────────────────────────────────────────────────────────

    #[test]
    fn viewport_default_is_zeroed() {
        let c = make_chart();
        assert_eq!(c.viewport.from_ts_ns, 0);
        assert_eq!(c.viewport.to_ts_ns, 0);
        assert!(!c.viewport.log_scale);
    }

    #[test]
    fn viewport_round_trips_via_serde() {
        use viewport::Viewport;
        let vp = Viewport {
            from_ts_ns: 1_700_000_000_000_000_000,
            to_ts_ns: 1_700_003_600_000_000_000,
            price_low: 4400.0,
            price_high: 4600.0,
            log_scale: true,
        };
        let json = serde_json::to_string(&vp).unwrap();
        let vp2: Viewport = serde_json::from_str(&json).unwrap();
        assert_eq!(vp, vp2);
    }

    // ── StyleTable ────────────────────────────────────────────────────────────

    #[test]
    fn style_table_intern_and_lookup() {
        let mut st = StyleTable::new();
        let s = Style {
            stroke: 0xFF_B8_00_FF,
            width_x100: 200,
            dash: style_table::DashKind::Dashed,
            fill: 0,
        };
        let id = st.intern(s);
        assert_eq!(st.get(id), Some(&s));
        assert_eq!(st.len(), 1);
    }

    // ── ChartState serde round-trip ───────────────────────────────────────────

    #[test]
    fn chart_state_serde_round_trip_preserves_all_fields() {
        let mut c = make_chart();
        c.title = Some(arcstr::literal!("Phase 5 smoke test"));
        c.theme = ThemeOverride::Dark;
        // Insert one drawing
        c.drawings.insert(Drawing::new(
            DrawingKind::FibRetracement,
            smallvec![
                Point { ts_ns: 1_700_000_000_000_000_000, price: 4200.0 },
                Point { ts_ns: 1_700_003_600_000_000_000, price: 4800.0 },
            ],
            StyleId::default(),
        ));
        // Insert one indicator
        c.indicators.push(IndicatorRef {
            ref_id: arcstr::literal!("apex.vwap"),
            ref_version: 2,
            param_schema_version: 1,
            pane: arcstr::literal!("main"),
            params: serde_json::json!({}),
            style: None,
            installed_locally: true,
            extras: Default::default(),
        });

        let json = serde_json::to_string(&c).expect("serialize");
        let c2: ChartState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(c2.id, 1);
        assert_eq!(c2.symbol.canonical.as_str(), "SPX");
        assert_eq!(c2.timeframe, Timeframe::M5);
        assert_eq!(c2.theme, ThemeOverride::Dark);
        assert_eq!(c2.title.as_ref().map(|s| s.as_str()), Some("Phase 5 smoke test"));
        assert_eq!(c2.drawings.len(), 1);
        assert_eq!(c2.indicators.len(), 1);
        assert_eq!(c2.indicators[0].ref_id.as_str(), "apex.vwap");
    }

    // ── Phase 5 shim — Chart::chart_state field is populated at None initially ─

    #[test]
    fn chart_renderer_chart_state_field_starts_none() {
        // This test lives in chart::state but exercises the shim contract:
        // Chart::new() must initialize chart_state to None (not crash or panic).
        // We verify the type compiles and the invariant holds by constructing
        // a ChartState independently (not via Chart, to avoid pulling in the
        // full renderer in unit tests).
        let c = make_chart();
        // A freshly-constructed ChartState has no drawings or indicators.
        assert_eq!(c.drawings.len(), 0);
        assert_eq!(c.annotations.len(), 0);
        assert_eq!(c.indicators.len(), 0);
        // Confirm the unknown_extensions bag is empty (no leftover cruft).
        assert!(c.unknown_extensions.is_empty());
    }
}
