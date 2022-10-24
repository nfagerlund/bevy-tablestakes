use std::f32::consts::*;
use bevy::prelude::Vec2;

// Mapping # of directional animation variants to discrete direction usage:
// - 1 (east) -- horizontal() and set flip if west.
// - 2 (east, west) -- horizontal(). (Would I ever do this?)
// - 3 (east, north, south) -- cardinal() and set flip if west.
// - 4 (east, north, west, south) -- cardinal().
// - 5 (east, northeast, north, south, southeast) -- ordinal() and
//     set flip if there's a west component.
// - 8 -- ordinal().

#[derive(Debug, PartialEq, Eq)]
pub enum Dir {
    E, N, W, S,
    NE, NW, SW, SE,
    Neutral,
}

impl Dir {
    /// Given a Vec2, return east, west, or neutral. Bias towards east when
    /// given exactly north or south.
    pub fn horizontal(motion: Vec2) -> Self {
        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            Self::Neutral
        } else if motion.x < 0.0 {
            Self::W
        } else {
            Self::E
        }
    }

    /// Given a Vec2, return north, south, or neutral. Bias towards south when
    /// given exactly east or west.
    pub fn vertical(motion: Vec2) -> Self {
        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            Self::Neutral
        } else if motion.y > 0.0 {
            Self::N
        } else {
            Self::S
        }
    }

    /// Given a Vec2, return one of the four cardinal directions or neutral.
    /// Bias towards horizontal when given an exact diagonal.
    pub fn cardinal(motion: Vec2) -> Self {
        const NE: f32 = FRAC_PI_4;
        const NW: f32 = 3.0 * FRAC_PI_4;
        const SW: f32 = -3.0 * FRAC_PI_4;
        const SE: f32 = -FRAC_PI_4;

        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            return Self::Neutral;
        }
        let angle = Vec2::X.angle_between(motion);
        if angle >= SE && angle <= NE {
            Self::E
        } else if angle > NE && angle < NW {
            Self::N
        } else if angle >= NW || angle <= SW { // the negative flip-over
            Self::W
        } else if angle > SW && angle < SE {
            Self::S
        } else {
            panic!("IDK what happened, but some angle didn't match a dir: {}", angle)
        }
    }

    /// Given a Vec2, return one of eight directions, or neutral. Bias when
    /// given an exact inter-intercardinal direction is ~whatever,~ bc you can't
    /// get your analog inputs exact enough to notice it.
    pub fn ordinal(motion: Vec2) -> Self {
        const ENE: f32 = 1.0 * FRAC_PI_8;
        const NNE: f32 = 3.0 * FRAC_PI_8;
        const NNW: f32 = 5.0 * FRAC_PI_8;
        const WNW: f32 = 7.0 * FRAC_PI_8;
        const WSW: f32 = -7.0 * FRAC_PI_8;
        const SSW: f32 = -5.0 * FRAC_PI_8;
        const SSE: f32 = -3.0 * FRAC_PI_8;
        const ESE: f32 = -1.0 * FRAC_PI_8;
        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            return Self::Neutral;
        }
        let angle = Vec2::X.angle_between(motion);
        if angle > ESE && angle <= ENE {
            Self::E
        } else if angle > ENE && angle <= NNE {
            Self::NE
        } else if angle > NNE && angle <= NNW {
            Self::N
        } else if angle > NNW && angle <= WNW {
            Self::NW
        } else if angle > WNW || angle <= WSW { // the negative flip-over
            Self::W
        } else if angle > WSW && angle <= SSW {
            Self::SW
        } else if angle > SSW && angle <= SSE {
            Self::S
        } else if angle > SSE && angle <= ESE {
            Self::SE
        } else {
            panic!("IDK what happened, but some angle didn't match a dir: {}", angle)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HARD_NE: Vec2 = Vec2::new(1.0, 1.0);
    const HARD_NW: Vec2 = Vec2::new(-1.0, 1.0);
    const HARD_SE: Vec2 = Vec2::new(1.0, -1.0);
    const HARD_SW: Vec2 = Vec2::new(-1.0, -1.0);

    const LIL_BIT: f32 = 0.0001;

    #[test]
    fn test_horizontal_from_vec2() {
        assert_eq!(Dir::horizontal(HARD_NE), Dir::E);
        assert_eq!(Dir::horizontal(Vec2::new(LIL_BIT, 1.0)), Dir::E);
        assert_eq!(Dir::horizontal(Vec2::new(-LIL_BIT, 1.0)), Dir::W);
        // on the deciding line:
        assert_eq!(Dir::horizontal(Vec2::new(0.0, 1.0)), Dir::E);
        // Blank or bogus input:
        assert_eq!(Dir::horizontal(Vec2::ZERO), Dir::Neutral);
        assert_eq!(Dir::horizontal(Vec2::new(f32::NAN, 1.0)), Dir::Neutral);
        assert_eq!(Dir::horizontal(Vec2::new(1.0, f32::INFINITY)), Dir::Neutral);
        assert_eq!(Dir::horizontal(Vec2::new(f32::NEG_INFINITY, 1.0)), Dir::Neutral);
    }

    #[test]
    fn test_vertical_from_vec2() {
        assert_eq!(Dir::vertical(HARD_NE), Dir::N);
        assert_eq!(Dir::vertical(Vec2::new(1.0, LIL_BIT)), Dir::N);
        assert_eq!(Dir::vertical(Vec2::new(1.0, -LIL_BIT)), Dir::S);
        // on the deciding line:
        assert_eq!(Dir::vertical(Vec2::new(-1.0, 0.0)), Dir::S);
        // Blank or bogus input:
        assert_eq!(Dir::vertical(Vec2::ZERO), Dir::Neutral);
        assert_eq!(Dir::vertical(Vec2::new(f32::NAN, 1.0)), Dir::Neutral);
        assert_eq!(Dir::vertical(Vec2::new(1.0, f32::INFINITY)), Dir::Neutral);
        assert_eq!(Dir::vertical(Vec2::new(f32::NEG_INFINITY, 1.0)), Dir::Neutral);
    }

    #[test]
    fn test_cardinal_from_vec2() {
        // Truly easy cases:
        assert_eq!(Dir::cardinal(Vec2::new(1.0, 0.0)), Dir::E);
        assert_eq!(Dir::cardinal(Vec2::new(1.0, LIL_BIT)), Dir::E);
        assert_eq!(Dir::cardinal(Vec2::new(1.0, -LIL_BIT)), Dir::E);
        assert_eq!(Dir::cardinal(Vec2::new(-1.0, 0.0)), Dir::W);
        assert_eq!(Dir::cardinal(Vec2::new(-1.0, LIL_BIT)), Dir::W);
        assert_eq!(Dir::cardinal(Vec2::new(-1.0, -LIL_BIT)), Dir::W);
        assert_eq!(Dir::cardinal(Vec2::new(0.0, 1.0)), Dir::N);
        assert_eq!(Dir::cardinal(Vec2::new(LIL_BIT, 1.0)), Dir::N);
        assert_eq!(Dir::cardinal(Vec2::new(-LIL_BIT, 1.0)), Dir::N);
        assert_eq!(Dir::cardinal(Vec2::new(0.0, -1.0)), Dir::S);
        assert_eq!(Dir::cardinal(Vec2::new(LIL_BIT, -1.0)), Dir::S);
        assert_eq!(Dir::cardinal(Vec2::new(-LIL_BIT, -1.0)), Dir::S);

        // Clear-cut cases (inter-intercardinal):
        // inter-intercardinal x/y components
        let iic_short: f32 = FRAC_PI_8.sin();
        let iic_long: f32 = FRAC_PI_8.cos();
        assert_eq!(Dir::cardinal(Vec2::new(iic_long, iic_short)), Dir::E);
        assert_eq!(Dir::cardinal(Vec2::new(iic_short, iic_long)), Dir::N);
        assert_eq!(Dir::cardinal(Vec2::new(-iic_short, iic_long)), Dir::N);
        assert_eq!(Dir::cardinal(Vec2::new(-iic_long, iic_short)), Dir::W);
        assert_eq!(Dir::cardinal(Vec2::new(-iic_long, -iic_short)), Dir::W);
        assert_eq!(Dir::cardinal(Vec2::new(-iic_short, -iic_long)), Dir::S);
        assert_eq!(Dir::cardinal(Vec2::new(iic_short, -iic_long)), Dir::S);
        assert_eq!(Dir::cardinal(Vec2::new(iic_long, -iic_short)), Dir::E);

        // Edge cases (hard ordinals):
        assert_eq!(Dir::cardinal(HARD_NE), Dir::E);
        assert_eq!(Dir::cardinal(HARD_NW), Dir::W);
        assert_eq!(Dir::cardinal(HARD_SW), Dir::W);
        assert_eq!(Dir::cardinal(HARD_SE), Dir::E);

        // Blank or bogus input:
        assert_eq!(Dir::cardinal(Vec2::ZERO), Dir::Neutral);
        assert_eq!(Dir::cardinal(Vec2::new(f32::NAN, 1.0)), Dir::Neutral);
        assert_eq!(Dir::cardinal(Vec2::new(1.0, f32::INFINITY)), Dir::Neutral);
        assert_eq!(Dir::cardinal(Vec2::new(f32::NEG_INFINITY, 1.0)), Dir::Neutral);
    }

    #[test]
    fn test_cardinal_ordinal_from_vec2() {
        assert_eq!(Dir::ordinal(HARD_NE), Dir::NE);
        assert_eq!(Dir::ordinal(HARD_NW), Dir::NW);
        assert_eq!(Dir::ordinal(HARD_SE), Dir::SE);
        assert_eq!(Dir::ordinal(HARD_SW), Dir::SW);

        // inter-intercardinal x/y components
        let iic_short: f32 = FRAC_PI_8.sin();
        let iic_long: f32 = FRAC_PI_8.cos();

        // On _just_ one side or the other of the deciding line:
        assert_eq!(Dir::ordinal(Vec2::new(iic_long + LIL_BIT, iic_short)), Dir::E);
        assert_eq!(Dir::ordinal(Vec2::new(iic_long, iic_short + LIL_BIT)), Dir::NE);
        assert_eq!(Dir::ordinal(Vec2::new(iic_long + LIL_BIT, -iic_short)), Dir::E);
        assert_eq!(Dir::ordinal(Vec2::new(iic_long, -(iic_short + LIL_BIT))), Dir::SE);

        // On exactly the deciding line:
        match Dir::ordinal(Vec2::new(iic_long, iic_short)) {
            Dir::E => (),
            Dir::NE => (),
            _ => {
                panic!("pi/8 angle should be either E or NE (don't care which)");
            }
        }

        // Blank or bogus input:
        assert_eq!(Dir::ordinal(Vec2::ZERO), Dir::Neutral);
        assert_eq!(Dir::ordinal(Vec2::new(f32::NAN, 1.0)), Dir::Neutral);
        assert_eq!(Dir::ordinal(Vec2::new(1.0, f32::INFINITY)), Dir::Neutral);
        assert_eq!(Dir::ordinal(Vec2::new(f32::NEG_INFINITY, 1.0)), Dir::Neutral);
    }
}
