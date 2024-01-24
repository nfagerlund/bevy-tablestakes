use bevy::prelude::{Rect, Vec2};

pub mod countup_timer;

/// Invert the Y coordinates of a Vec2
pub fn flip_vec2_y(v: Vec2) -> Vec2 {
    Vec2::new(v.x, -(v.y))
}

/// Invert the X coordinates of a Vec2
pub fn flip_vec2_x(v: Vec2) -> Vec2 {
    Vec2::new(-(v.x), v.y)
}

/// Mirror a Rect vertically, around the implied origin point that the corners
/// are defined relative to.
pub fn flip_rect_y(r: Rect) -> Rect {
    // Note that mirroring each corner yields valid opposite corners in the new
    // mirrored rectangle, but they're the top left and bottom right, rather
    // than the bottom left (min) and top right (max). (Or, vice-versa if
    // you're using a top-down Y coordinate. you know what I mean.)
    // Anyway, Rect::from_corners can sort that out.
    Rect::from_corners(flip_vec2_y(r.min), flip_vec2_y(r.max))
}

/// Mirror a Rect horizontally around the implied origin.
pub fn flip_rect_x(r: Rect) -> Rect {
    Rect::from_corners(flip_vec2_x(r.min), flip_vec2_x(r.max))
}

/// Translate a Rect so its corners are relative to a provided origin/anchor/pivot
/// point.
pub fn move_rect_origin(r: Rect, origin: Vec2) -> Rect {
    Rect {
        min: r.min - origin,
        max: r.max - origin,
    }
}

// Determines whether an input Vec2 no longer has any movement component in a given cardinal direction.
pub fn turned_away_from(cardinal: Vec2, input: Vec2) -> bool {
    if cardinal.x == 0.0 {
        cardinal.y.signum() != input.y.signum()
    } else {
        cardinal.x.signum() != input.x.signum()
    }
}
