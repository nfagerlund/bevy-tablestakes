use std::f32::consts::*;
use bevy::prelude::Vec2;

// Mapping # of directional variants to discretedir usage:
// - 1 (east) -- horizontal_from_vec2 and set flip if west.
// - 2 (east, west) -- horizontal_from_vec2. (Would I ever do this?)
// - 3 (east, north, south) -- cardinal_from_vec2 and set flip if west.
// - 4 (east, north, west, south) -- cardinal_from_vec2.
// - 5 (east, northeast, north, south, southeast) -- cardinal_ordinal_from_vec2 and
//     set flip if there's a west component.
// - 8 -- cardinal_ordinal_from_vec2

#[derive(Debug, PartialEq, Eq)]
pub enum DiscreteDir {
    E, N, W, S,
    NE, NW, SW, SE,
    Neutral,
}

impl DiscreteDir {
    // Given a Vec2, return east, west, or neutral. Bias towards east.
    pub fn horizontal_from_vec2(motion: Vec2) -> Self {
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

    // Given a Vec2, return north, south, or neutral. Bias towards south.
    pub fn vertical_from_vec2(motion: Vec2) -> Self {
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

    // Given a Vec2, return one of the four cardinal directions or neutral. Bias
    // towards horizontal.
    pub fn cardinal_from_vec2(motion: Vec2) -> Self {
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

    // Given a Vec2, return one of the four cardinal directions, one of the four
    // ordinal/intercardinal directions, or neutral. Bias is whatever bc you
    // can't get your analog inputs exact enough to notice it.
    pub fn cardinal_ordinal_from_vec2(motion: Vec2) -> Self {
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
        assert_eq!(DiscreteDir::horizontal_from_vec2(HARD_NE), DiscreteDir::E);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(LIL_BIT, 1.0)), DiscreteDir::E);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(-LIL_BIT, 1.0)), DiscreteDir::W);
        // on the deciding line:
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(0.0, 1.0)), DiscreteDir::E);
        // Blank or bogus input:
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }

    #[test]
    fn test_vertical_from_vec2() {
        assert_eq!(DiscreteDir::vertical_from_vec2(HARD_NE), DiscreteDir::N);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(1.0, LIL_BIT)), DiscreteDir::N);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(1.0, -LIL_BIT)), DiscreteDir::S);
        // on the deciding line:
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(-1.0, 0.0)), DiscreteDir::S);
        // Blank or bogus input:
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }

    #[test]
    fn test_cardinal_from_vec2() {
        // Truly easy cases:
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, 0.0)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, LIL_BIT)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, -LIL_BIT)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-1.0, 0.0)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-1.0, LIL_BIT)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-1.0, -LIL_BIT)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(0.0, 1.0)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(LIL_BIT, 1.0)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-LIL_BIT, 1.0)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(0.0, -1.0)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(LIL_BIT, -1.0)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-LIL_BIT, -1.0)), DiscreteDir::S);

        // Clear-cut cases (inter-intercardinal):
        // inter-intercardinal x/y components
        let iic_short: f32 = FRAC_PI_8.sin();
        let iic_long: f32 = FRAC_PI_8.cos();
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_long, iic_short)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_short, iic_long)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_short, iic_long)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_long, iic_short)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_long, -iic_short)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_short, -iic_long)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_short, -iic_long)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_long, -iic_short)), DiscreteDir::E);

        // Edge cases (hard ordinals):
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_NE), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_NW), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_SW), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_SE), DiscreteDir::E);

        // Blank or bogus input:
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }

    #[test]
    fn test_cardinal_ordinal_from_vec2() {
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_NE), DiscreteDir::NE);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_NW), DiscreteDir::NW);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_SE), DiscreteDir::SE);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_SW), DiscreteDir::SW);

        // inter-intercardinal x/y components
        let iic_short: f32 = FRAC_PI_8.sin();
        let iic_long: f32 = FRAC_PI_8.cos();

        // On _just_ one side or the other of the deciding line:
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long + LIL_BIT, iic_short)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long, iic_short + LIL_BIT)), DiscreteDir::NE);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long + LIL_BIT, -iic_short)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long, -(iic_short + LIL_BIT))), DiscreteDir::SE);

        // On exactly the deciding line:
        match DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long, iic_short)) {
            DiscreteDir::E => (),
            DiscreteDir::NE => (),
            _ => {
                panic!("pi/8 angle should be either E or NE (don't care which)");
            }
        }

        // Blank or bogus input:
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }
}
