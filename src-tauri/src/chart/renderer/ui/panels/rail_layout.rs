//! Rail stacking layout — the auto-column packing for side-rail panels.
//!
//! Each open panel declares a [`RailHeight`] (Full or Half — binary, no thirds).
//! Panels auto-pack into **columns**: a column holds one Full, or two stacked
//! Halves (greedy first-fit, insertion order). Within a column the panels fill
//! it proportionally — a lone Half fills its column, two Halves split 50/50.
//! Columns sit side by side; the rail's width grows one column-width per column.
//!
//! This is a **rail-specific** system (vertical-only, sidebar semantics) — it
//! deliberately does NOT reuse the chart workspace's recursive `PaneGrid`.
//!
//! Heights are tracked in integer "halves" (Full=2, Half=1; column capacity = 2)
//! so packing is exact with no float drift. This matches the footer's flat
//! half/full split model — one shared binary granularity across docked regions.

/// How tall a panel wants to be within its rail column. Binary only — a column
/// holds one Full panel, or two stacked Halves. No thirds (matches the footer's
/// flat half/full tiling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailHeight {
    /// Full height — occupies a whole column alone.
    Full,
    /// Half height — pairs with another Half (two stacked per column).
    Half,
}

const COLUMN_UNITS: u8 = 2;

impl RailHeight {
    /// Height in halves of a column (Full = 2, Half = 1).
    #[inline]
    pub fn units(self) -> u8 {
        match self {
            RailHeight::Full => 2,
            RailHeight::Half => 1,
        }
    }

    /// Split toggle: Full ⇄ Half.
    #[inline]
    pub fn next(self) -> RailHeight {
        match self {
            RailHeight::Full => RailHeight::Half,
            RailHeight::Half => RailHeight::Full,
        }
    }

    /// Short label for UI.
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            RailHeight::Full => "1",
            RailHeight::Half => "½",
        }
    }
}

/// One column of stacked panels (indices into the input slice, top→bottom).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailColumn {
    /// Panel indices in this column, top to bottom.
    pub members: Vec<usize>,
    /// Total occupied units (≤ 2).
    pub used_units: u8,
}

/// Greedy first-fit pack of panel heights into columns. Returns columns in
/// left-to-right order; each column's `members` are top-to-bottom.
///
/// Algorithm: for each panel (in order), place it in the FIRST existing column
/// where it still fits (`used + units ≤ 2`); otherwise open a new column to the
/// right. This makes `[Full, Half, Half]` → `[[Full]], [[Half, Half]]` and keeps
/// insertion order stable.
pub fn pack_columns(heights: &[RailHeight]) -> Vec<RailColumn> {
    let mut cols: Vec<RailColumn> = Vec::new();
    for (idx, h) in heights.iter().enumerate() {
        let u = h.units();
        // First column that still has room.
        let slot = cols.iter_mut().find(|c| c.used_units + u <= COLUMN_UNITS);
        match slot {
            Some(c) => { c.members.push(idx); c.used_units += u; }
            None => cols.push(RailColumn { members: vec![idx], used_units: u }),
        }
    }
    cols
}

/// A computed rect for one panel: `(panel_index, x, y, w, h)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailPanelRect {
    pub panel: usize,
    pub rect: egui::Rect,
}

/// Compute per-panel rects for a rail. `origin` is the rail's top-left, growing
/// right for `side == Right` (or you pass a right-aligned origin for the right
/// rail). `col_width` is one column's width; `rail_height` the full vertical
/// extent. Within a column, panels fill proportionally to their units.
pub fn compute_rail_rects(
    heights: &[RailHeight],
    origin: egui::Pos2,
    col_width: f32,
    gap: f32,
    rail_height: f32,
) -> (Vec<RailPanelRect>, f32 /* total rail width */) {
    let cols = pack_columns(heights);
    let mut out = Vec::with_capacity(heights.len());
    let mut x = origin.x;
    for col in &cols {
        // Heights are ABSOLUTE fractions of the column (Full = 1, Half = ½),
        // NOT normalized to the column's contents — so a lone Half genuinely
        // occupies the top half and leaves the bottom half open (clear visual
        // feedback that the panel is split, and room for a second instance).
        let inner_h = (rail_height - gap * (col.members.len().saturating_sub(1)) as f32).max(0.0);
        let mut y = origin.y;
        for &i in &col.members {
            let frac = heights[i].units() as f32 / COLUMN_UNITS as f32;
            let h = (inner_h * frac).max(0.0);
            out.push(RailPanelRect {
                panel: i,
                rect: egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(col_width, h)),
            });
            y += h + gap;
        }
        x += col_width + gap;
    }
    let total_w = if cols.is_empty() { 0.0 } else { x - origin.x - gap };
    (out, total_w.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use RailHeight::*;

    fn cols_of(hs: &[RailHeight]) -> Vec<Vec<usize>> {
        pack_columns(hs).into_iter().map(|c| c.members).collect()
    }

    #[test]
    fn two_halves_stack_in_one_column() {
        assert_eq!(cols_of(&[Half, Half]), vec![vec![0, 1]]);
    }

    #[test]
    fn full_plus_two_halves_makes_two_columns() {
        // Full → own column; the two Halves stack beside it.
        assert_eq!(cols_of(&[Full, Half, Half]), vec![vec![0], vec![1, 2]]);
    }

    #[test]
    fn three_halves_overflow_to_second_column() {
        // Two halves fill col1 (cap 2); the third starts col2.
        assert_eq!(cols_of(&[Half, Half, Half]), vec![vec![0, 1], vec![2]]);
    }

    #[test]
    fn fulls_each_get_their_own_column() {
        assert_eq!(cols_of(&[Full, Full]), vec![vec![0], vec![1]]);
    }

    #[test]
    fn full_after_half_opens_new_column() {
        // Half occupies 1 unit in col1; a Full (2 units) can't fit → new column.
        assert_eq!(cols_of(&[Half, Full]), vec![vec![0], vec![1]]);
    }

    #[test]
    fn lone_half_occupies_half_its_column() {
        let (rects, w) = compute_rail_rects(&[Half], egui::pos2(0.0, 0.0), 300.0, 0.0, 600.0);
        assert_eq!(rects.len(), 1);
        // A lone Half takes the TOP half (300 of 600) — bottom half left open
        // for visual feedback / a second instance.
        assert!((rects[0].rect.height() - 300.0).abs() < 0.5);
        assert!((w - 300.0).abs() < 0.5);
    }

    #[test]
    fn lone_full_fills_its_column() {
        let (rects, _w) = compute_rail_rects(&[Full], egui::pos2(0.0, 0.0), 300.0, 0.0, 600.0);
        assert!((rects[0].rect.height() - 600.0).abs() < 0.5);
    }

    #[test]
    fn two_halves_split_5050() {
        let (rects, _w) = compute_rail_rects(&[Half, Half], egui::pos2(0.0, 0.0), 300.0, 0.0, 600.0);
        assert!((rects[0].rect.height() - 300.0).abs() < 0.5);
        assert!((rects[1].rect.height() - 300.0).abs() < 0.5);
    }

    #[test]
    fn full_plus_halves_two_column_widths() {
        let (_r, w) = compute_rail_rects(&[Full, Half, Half], egui::pos2(0.0, 0.0), 300.0, 8.0, 600.0);
        // 2 columns → ~2 widths + 1 gap.
        assert!((w - (300.0 * 2.0 + 8.0)).abs() < 0.5);
    }
}
