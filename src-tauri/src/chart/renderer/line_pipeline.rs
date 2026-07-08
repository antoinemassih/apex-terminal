//! WS-E E5 groundwork — the unified line hit-test spec (Phase 1: additive,
//! render path UNCHANGED).
//!
//! The chart draws five kinds of draggable horizontal price line — alert, play,
//! order, drawing, trigger — and today each is hit-tested by its own closure
//! inside the 13k-line render fn (`render/pane/core.rs`) with subtly DIFFERENT
//! rules and a hand-wired if-else priority chain:
//!   - Alert : 10px, inclusive (`<=`), no price-axis gate     (core.rs:8812)
//!   - Play  : 14px, exclusive (`<`),  left-of-axis gate       (core.rs:7879)
//!   - Order : 14px, exclusive (`<`),  left-of-axis gate       (core.rs:7849)
//!             + a badge-area exclusion the caller applies separately
//!   - Drawing (HLine): 12px, exclusive (`<`)                  (hit_drawing arm)
//!   - Trigger: render-only today — no hit/drag implemented.
//!   - Priority when several overlap: Alert > Play > Order > Drawing > pan
//!                                                            (core.rs:8808)
//!
//! This module captures that behavior AS DATA so the divergence is reviewable
//! and unit-tested in one place, WITHOUT changing the render path. It is the
//! foundation for the E5 unification: a later phase shadow-asserts the render
//! closures against this table (debug-only, compiled out of release), then
//! switches the render fn to consume it. Only then is the divergence a
//! deliberate design choice rather than accreted inconsistency. See
//! docs/REMEDIATION_PLAN.md E5 ("adding a line kind = one enum variant + one
//! style row").

/// The five draggable price-line kinds, in the priority order the render fn's
/// drag-pick chain uses. Trigger is included for completeness (render-only
/// today). The enum — not the `priority()` discriminant — is the source of
/// truth; priority is expressed explicitly below so reordering is a one-line
/// change, not an enum reshuffle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    Alert,
    Play,
    Order,
    Drawing,
    Trigger,
}

/// Vertical hit band around a horizontal line. `tol` is the pixel radius;
/// `inclusive` selects `<=` vs `<` at the exact boundary (alert lines are
/// inclusive, the rest exclusive — a real current divergence). `left_of_axis`
/// records whether the kind's hit rule additionally requires the pointer to be
/// left of the price axis (play/order do; alert/drawing do not).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HitRule {
    pub tol: f32,
    pub inclusive: bool,
    pub left_of_axis: bool,
}

impl HitRule {
    /// Does a pointer at `pointer_y` fall within this band of a line drawn at
    /// `line_y`? Vertical test only — the caller applies `left_of_axis` against
    /// the pointer x (and, for orders, the badge-area exclusion).
    pub fn hits_vertical(&self, pointer_y: f32, line_y: f32) -> bool {
        let d = (pointer_y - line_y).abs();
        if self.inclusive { d <= self.tol } else { d < self.tol }
    }
}

impl LineKind {
    /// Drag-pick priority: 0 = highest (checked first when several lines overlap
    /// under the pointer). Mirrors the render fn's `alert > play > order >
    /// drawing` if-else chain (core.rs:8808).
    pub fn priority(self) -> u8 {
        match self {
            LineKind::Alert => 0,
            LineKind::Play => 1,
            LineKind::Order => 2,
            LineKind::Drawing => 3,
            LineKind::Trigger => 4,
        }
    }

    /// The CURRENT (pre-unification) hit rule for this kind, exactly as the
    /// render closures apply it today. This is a spec of existing behavior, not
    /// a proposal — the divergent tolerances/boundaries are intentional to
    /// record so the eventual unification is an explicit, reviewable decision.
    pub fn hit_rule(self) -> HitRule {
        match self {
            LineKind::Alert   => HitRule { tol: 10.0, inclusive: true,  left_of_axis: false },
            LineKind::Play    => HitRule { tol: 14.0, inclusive: false, left_of_axis: true  },
            LineKind::Order   => HitRule { tol: 14.0, inclusive: false, left_of_axis: true  },
            LineKind::Drawing => HitRule { tol: 12.0, inclusive: false, left_of_axis: false },
            LineKind::Trigger => HitRule { tol: 14.0, inclusive: false, left_of_axis: false },
        }
    }
}

/// Resolve overlapping hits the way the render fn's drag-pick chain does: among
/// the kinds whose line was hit under the pointer, return the highest-priority
/// one (Alert first). Passing `LineKind::Trigger` is accepted but it never wins
/// against an implemented kind.
pub(crate) fn resolve_pick(hits: &[LineKind]) -> Option<LineKind> {
    hits.iter().copied().min_by_key(|k| k.priority())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_rules_match_current_render_behavior() {
        // These MUST equal the literals in the render closures; if a closure's
        // tolerance changes, this test (and the shadow assert in a later phase)
        // is what catches the drift.
        assert_eq!(LineKind::Alert.hit_rule(),   HitRule { tol: 10.0, inclusive: true,  left_of_axis: false });
        assert_eq!(LineKind::Play.hit_rule(),    HitRule { tol: 14.0, inclusive: false, left_of_axis: true  });
        assert_eq!(LineKind::Order.hit_rule(),   HitRule { tol: 14.0, inclusive: false, left_of_axis: true  });
        assert_eq!(LineKind::Drawing.hit_rule(), HitRule { tol: 12.0, inclusive: false, left_of_axis: false });
    }

    #[test]
    fn boundary_inclusive_vs_exclusive() {
        // Alert is inclusive: a pointer exactly 10px away still hits.
        assert!(LineKind::Alert.hit_rule().hits_vertical(100.0, 110.0));
        assert!(!LineKind::Alert.hit_rule().hits_vertical(100.0, 110.5));
        // Play/Order are exclusive: exactly 14px away does NOT hit.
        assert!(!LineKind::Play.hit_rule().hits_vertical(100.0, 114.0));
        assert!(LineKind::Play.hit_rule().hits_vertical(100.0, 113.5));
        // Order matches play's band (render comment: "same tolerance as orders").
        assert_eq!(LineKind::Order.hit_rule().tol, LineKind::Play.hit_rule().tol);
    }

    #[test]
    fn hits_vertical_is_symmetric() {
        let r = LineKind::Order.hit_rule();
        assert_eq!(r.hits_vertical(100.0, 105.0), r.hits_vertical(105.0, 100.0));
    }

    #[test]
    fn priority_matches_render_chain() {
        // alert > play > order > drawing (core.rs:8808), trigger last.
        assert!(LineKind::Alert.priority() < LineKind::Play.priority());
        assert!(LineKind::Play.priority() < LineKind::Order.priority());
        assert!(LineKind::Order.priority() < LineKind::Drawing.priority());
        assert!(LineKind::Drawing.priority() < LineKind::Trigger.priority());
    }

    #[test]
    fn resolve_pick_prefers_highest_priority() {
        assert_eq!(resolve_pick(&[LineKind::Drawing, LineKind::Alert, LineKind::Order]), Some(LineKind::Alert));
        assert_eq!(resolve_pick(&[LineKind::Order, LineKind::Play]), Some(LineKind::Play));
        assert_eq!(resolve_pick(&[LineKind::Drawing, LineKind::Order]), Some(LineKind::Order));
        assert_eq!(resolve_pick(&[]), None);
    }
}
