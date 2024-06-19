use bevy::asset::Asset;
use bevy::asset::Handle;
use bevy::math::{prelude::*, Rect};
use bevy::prelude::{Component, Entity, Event};
use bevy::reflect::Reflect;
use bevy::reflect::TypePath;
use bevy::sprite::TextureAtlas;
use bevy::utils::Duration;
use std::collections::HashMap;

use crate::compass::{self};
use crate::toolbox::countup_timer::CountupTimer;

#[derive(Asset, Debug, TypePath)]
pub struct CharAnimation {
    pub variants: VariantsMap,
    pub directionality: Directionality,
    pub texture_atlas: Handle<TextureAtlas>,
}

#[derive(Debug)]
pub struct CharAnimationVariant {
    pub name: VariantName,
    pub frames: Vec<CharAnimationFrame>,
    pub duration: Duration,
}

impl CharAnimationVariant {
    /// Returns the final frame time to use for the requested frame, after
    /// applying the specified FrameTimeOverride behavior.
    pub fn resolved_frame_time(
        &self,
        frame_index: usize,
        override_type: FrameTimeOverride,
    ) -> Duration {
        let frame_duration = self.raw_frame_time(frame_index);
        match override_type {
            FrameTimeOverride::None => frame_duration,
            FrameTimeOverride::Ms(millis) => Duration::from_millis(millis),
            FrameTimeOverride::Scale(scale) => frame_duration.mul_f32(scale),
            FrameTimeOverride::TotalMs(total_millis) => {
                // Split the total milliseconds between the frames, proportional to the fraction of
                // the total duration they occupy.
                let millis = total_millis * (frame_duration.as_millis() as u64)
                    / (self.duration.as_millis() as u64); // Integer division is fine
                Duration::from_millis(millis)
            },
        }
    }

    #[inline]
    pub fn raw_frame_time(&self, frame_index: usize) -> Duration {
        self.frames[frame_index].duration
    }
}

// I've changed my mind about tags: I'm gonna start out with a rigid enum,
// enforce tag names at load time, and expand behavior as needed later. Duping
// the Dir type for easier refactors later??
pub type VariantName = compass::Dir;
pub type VariantsMap = HashMap<VariantName, CharAnimationVariant>;

/// The known kinds of sprite variation for representing different directions.
#[derive(Debug, Reflect)]
pub enum Directionality {
    Zero, // Neutral
    OneE, // E (animal, flip for W)
    // OneN,  // N (spaceship, flip for S)
    // TwoH,  // E, W
    // Three, // E, N, S (flip for W)
    Four, // E, N, W, S
          // Five,  // E, NE, N, S, SE (flip for W, NW, SW)
          // Eight, // 💪🏽💪🏽💪🏽
}

/// Data for an individual animation frame. This struct contains coordinates for
/// some points and rectangles. The points have some particular frame of
/// reference (described in comments), but the rectangles are all relative to
/// the origin point and laid out in Bevy spatial coordinate space (y-up).
#[derive(Debug)]
pub struct CharAnimationFrame {
    /// Index into the `TextureAtlas`.
    pub index: usize,
    /// Frame duration.
    pub duration: Duration,
    /// Origin point of the sprite in pixel coordinates, where 0,0 is top left.
    /// Stored on the frame for convenience, but shouldn't vary.
    pub origin: Vec2,
    /// Anchor point that can be assigned to a TextureAtlasSprite. Same as
    /// origin, but transformed to normalized coordinates relative to the
    /// texture size (where 0,0 is the center and -.5,-.5 is bottom left).
    pub anchor: Vec2,
    /// Bbox for the projected foot position on the ground.
    pub walkbox: Option<Rect>,
    /// Bbox for the damage-dealing area of a frame.
    pub hitbox: Option<Rect>,
    /// Bbox for the damageable area of a frame.
    pub hurtbox: Option<Rect>,
}

#[derive(Event)]
pub struct AnimateFinishedEvent(pub Entity);

#[derive(Component, Debug)]
pub struct CharAnimationState {
    pub animation: Handle<CharAnimation>,
    pub variant: Option<VariantName>,
    // Whether to flip the sprite. Set at same time as variant, bc it's tied
    // to the directionality of the animation as a whole. Saved on state struct
    // bc of where we need to use it (animate_system).
    pub flip_x: bool,
    pub playback: Playback,
    pub frame: usize,
    // To start with, we'll just always loop.
    pub frame_timer: Option<CountupTimer>,
    /// Optionally override the animation's frame timings. Can set all
    /// frames to a uniform duration (in ms), split a given duration among all
    /// frames, or scale all frames by some factor.
    pub frame_time_override: FrameTimeOverride,
}

#[derive(Debug, Clone, Copy)]
pub enum Playback {
    Loop,
    Once,
}

/// Allow programmatically overriding the frame times from the animation source
/// data, for things like stretching out a motion to fit it to a particular
/// total duration.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum FrameTimeOverride {
    None,
    Ms(u64),
    Scale(f32),
    TotalMs(u64),
}

impl CharAnimationState {
    pub fn new(animation: Handle<CharAnimation>, variant: VariantName, playback: Playback) -> Self {
        CharAnimationState {
            animation,
            variant: Some(variant),
            flip_x: false,
            playback,
            // in the future I might end up wanting to blend between animations
            // at a particular frame. Doesn't matter yet tho.
            frame: 0,
            frame_timer: None,
            frame_time_override: FrameTimeOverride::None,
        }
    }

    // Restart the animation and wipe any state left over from the previous one.
    // An implementation detail of change_animation.
    fn reset(&mut self) {
        self.frame = 0;
        self.frame_timer = None;
        self.variant = None;
        self.frame_time_override = FrameTimeOverride::None;
    }

    /// Change direction of animation, unless it's already set to the requested one.
    /// Note that this DOESN'T restart the animation, it picks up right where the
    /// previous variant left off.
    pub fn change_variant(&mut self, variant: VariantName) {
        if self.variant != Some(variant) {
            self.variant = Some(variant);
        }
    }

    pub fn change_animation(&mut self, animation: Handle<CharAnimation>, playback: Playback) {
        if self.animation != animation {
            self.animation = animation;
            // TODO: I'm leaving the variant the same, but actually I don't know if
            // that's the right move -- question is whether I'd ever switch to an
            // animation with fewer variants.
            self.playback = playback;
            self.reset();
        }
    }

    pub fn _set_frame_times_to(&mut self, millis: u64) {
        self.frame_time_override = FrameTimeOverride::Ms(millis);
    }

    pub fn _scale_frame_times_by(&mut self, scale: f32) {
        self.frame_time_override = FrameTimeOverride::Scale(scale);
    }

    pub fn set_total_run_time_to(&mut self, millis: u64) {
        self.frame_time_override = FrameTimeOverride::TotalMs(millis);
    }

    pub fn timer_just_finished(&self) -> bool {
        matches!(
            self.frame_timer.as_ref().map(|t| t.just_finished()),
            Some(true)
        )
    }
}
