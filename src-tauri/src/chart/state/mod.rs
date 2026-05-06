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

#![allow(dead_code)]

pub mod codec;
pub mod commands;
pub mod drawings;
pub mod file_io;
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

    #[test]
    fn empty_chart_is_constructible() {
        let s = Symbol {
            canonical: arcstr::literal!("SPX"),
            asset_class: AssetClass::Index,
            provider_hints: ProviderHints::default(),
        };
        let c = ChartState::new(1, s, Timeframe::M5);
        assert_eq!(c.drawings.len(), 0);
        assert_eq!(c.indicators.len(), 0);
        assert!(c.template_id.is_none());
        assert!(c.unknown_extensions.is_empty());
    }
}
