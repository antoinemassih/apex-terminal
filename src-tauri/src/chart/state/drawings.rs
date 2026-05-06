//! Drawing primitives in canonical form.
//!
//! Anchoring rule (see XOL spec §5): every drawing point is `(ts_ns, price)`,
//! never bar index, never pixels. This is what lets a drawing saved on 5m
//! reopen correctly on 1m or 1h.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use slotmap::new_key_type;
use smallvec::SmallVec;

use super::extension_bag::ExtensionBag;
use super::style_table::StyleId;

new_key_type! {
    pub struct DrawingId;
}

/// A single anchor point. f32 prices are sufficient — the renderer can't
/// resolve to better than ~1px precision regardless. f64 is paid for at the
/// codec boundary (DB / XOL) when wider precision is needed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub ts_ns: i64,
    pub price: f32,
}

/// Drawing kind discriminant. Enum, not string, so the renderer dispatches
/// without allocation. Unknown kinds (from a newer XOL writer) ride along in
/// `ExtensionBag` and skip rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingKind {
    Trendline,
    HorizontalLine,
    VerticalLine,
    Ray,
    Rect,
    Ellipse,
    FibRetracement,
    FibExtension,
    Pitchfork,
    Text,
    Arrow,
    Polyline,
    Path,
}

bitflags! {
    /// Per-drawing flags packed into a u16 (saves 7 bytes vs separate bools).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct DrawingFlags: u16 {
        const VISIBLE      = 1 << 0;
        const LOCKED       = 1 << 1;
        const EXTEND_LEFT  = 1 << 2;
        const EXTEND_RIGHT = 1 << 3;
        const SHOW_LABELS  = 1 << 4;
    }
}

impl Default for DrawingFlags {
    fn default() -> Self {
        DrawingFlags::VISIBLE
    }
}

/// A drawing on the chart. Heterogeneous kinds (fib levels, text body, etc.)
/// live in `extras` (CBOR via the bag), so adding a new kind doesn't ripple
/// through the struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drawing {
    pub kind: DrawingKind,
    pub points: SmallVec<[Point; 4]>,
    pub style: StyleId,
    pub flags: DrawingFlags,
    pub z: i16,

    /// Kind-specific extras (fib `levels`, text `body`/`font_size`, pitchfork
    /// 3rd anchor, etc.) plus forward-compat for unknown fields from a newer
    /// XOL writer. Renderer reads what it knows; the rest survives round-trip.
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    pub extras: ExtensionBag,
}

impl Drawing {
    pub fn new(kind: DrawingKind, points: SmallVec<[Point; 4]>, style: StyleId) -> Self {
        Self {
            kind,
            points,
            style,
            flags: DrawingFlags::default(),
            z: 0,
            extras: ExtensionBag::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    #[test]
    fn trendline_uses_inline_points() {
        let style = StyleId::default();
        let d = Drawing::new(
            DrawingKind::Trendline,
            smallvec![
                Point { ts_ns: 1_700_000_000_000_000_000, price: 4520.5 },
                Point { ts_ns: 1_700_000_300_000_000_000, price: 4555.0 },
            ],
            style,
        );
        assert_eq!(d.points.len(), 2);
        assert!(!d.points.spilled(), "2-point trendline must stay inline");
        assert!(d.flags.contains(DrawingFlags::VISIBLE));
    }
}
