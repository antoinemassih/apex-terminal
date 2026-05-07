//! Overlay placement engine — shared by Tooltip, Popover, HoverCard, and
//! eventually Select. Picks an anchor point + flips the overlay if it
//! would overflow the screen.

use egui::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Default for Side {
    fn default() -> Self { Side::Bottom }
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Center,
    Start,
    End,
}

#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub side: Side,
    pub align: Align,
    /// Pixel gap between anchor and overlay.
    pub offset: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            side: Side::Bottom,
            align: Align::Center,
            offset: 6.0,
        }
    }
}

/// Compute the top-left of an overlay rect anchored to `anchor` according
/// to `placement`. If the overlay would overflow `screen` on the requested
/// side, flip to the opposite side. After flipping, clamp into `screen`.
/// Returns `(top_left, side_used)`.
pub fn compute(
    anchor: Rect,
    overlay_size: Vec2,
    placement: Placement,
    screen: Rect,
) -> (Pos2, Side) {
    let try_side = |side: Side| -> Pos2 { position_for(anchor, overlay_size, side, placement) };

    let primary_pos = try_side(placement.side);
    let primary_rect = Rect::from_min_size(primary_pos, overlay_size);

    let (mut pos, mut side) = if screen.contains_rect(primary_rect) {
        (primary_pos, placement.side)
    } else {
        let opp = placement.side.opposite();
        let alt_pos = try_side(opp);
        let alt_rect = Rect::from_min_size(alt_pos, overlay_size);
        if screen.contains_rect(alt_rect) {
            (alt_pos, opp)
        } else {
            // Neither fits perfectly — keep primary, will clamp below.
            (primary_pos, placement.side)
        }
    };

    // Clamp into screen.
    let max_x = (screen.right() - overlay_size.x).max(screen.left());
    let max_y = (screen.bottom() - overlay_size.y).max(screen.top());
    pos.x = pos.x.clamp(screen.left(), max_x);
    pos.y = pos.y.clamp(screen.top(), max_y);

    (pos, side)
}

fn position_for(anchor: Rect, size: Vec2, side: Side, p: Placement) -> Pos2 {
    let off = p.offset;
    match side {
        Side::Top => {
            let x = align_x(anchor, size, p.align);
            Pos2::new(x, anchor.top() - size.y - off)
        }
        Side::Bottom => {
            let x = align_x(anchor, size, p.align);
            Pos2::new(x, anchor.bottom() + off)
        }
        Side::Left => {
            let y = align_y(anchor, size, p.align);
            Pos2::new(anchor.left() - size.x - off, y)
        }
        Side::Right => {
            let y = align_y(anchor, size, p.align);
            Pos2::new(anchor.right() + off, y)
        }
    }
}

fn align_x(anchor: Rect, size: Vec2, align: Align) -> f32 {
    match align {
        Align::Start => anchor.left(),
        Align::End => anchor.right() - size.x,
        Align::Center => anchor.center().x - size.x * 0.5,
    }
}

fn align_y(anchor: Rect, size: Vec2, align: Align) -> f32 {
    match align {
        Align::Start => anchor.top(),
        Align::End => anchor.bottom() - size.y,
        Align::Center => anchor.center().y - size.y * 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))
    }

    #[test]
    fn bottom_fits_no_flip() {
        let anchor = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(40.0, 20.0));
        let size = Vec2::new(120.0, 60.0);
        let p = Placement { side: Side::Bottom, align: Align::Center, offset: 4.0 };
        let (_pos, side) = compute(anchor, size, p, screen());
        assert_eq!(side, Side::Bottom);
    }

    #[test]
    fn bottom_overflow_flips_to_top() {
        // anchor near bottom of screen — bottom overlay would overflow.
        let anchor = Rect::from_min_size(Pos2::new(100.0, 770.0), Vec2::new(40.0, 20.0));
        let size = Vec2::new(120.0, 60.0);
        let p = Placement { side: Side::Bottom, align: Align::Center, offset: 4.0 };
        let (_pos, side) = compute(anchor, size, p, screen());
        assert_eq!(side, Side::Top);
    }

    #[test]
    fn clamps_when_neither_side_fits() {
        let anchor = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
        let size = Vec2::new(120.0, 900.0); // taller than screen
        let p = Placement { side: Side::Bottom, align: Align::Center, offset: 4.0 };
        let (pos, _) = compute(anchor, size, p, screen());
        assert!(pos.y >= screen().top() - 0.001);
    }
}
