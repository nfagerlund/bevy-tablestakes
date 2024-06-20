use bevy::prelude::*;
use bevy::sprite::{Anchor, TextureAtlas};

use super::assets::*;
use super::types::*;
use crate::collision::{Hitbox, Hurtbox, Walkbox};
use crate::compass::Dir;
use crate::toolbox::countup_timer::CountupTimer;
use crate::toolbox::{flip_rect_x, flip_vec2_x};
use crate::Motion;

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharAnimationSystems;
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpriteChangers;

pub struct CharAnimationPlugin;

impl Plugin for CharAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<CharAnimation>()
            .init_asset_loader::<CharAnimationLoader>()
            .add_event::<AnimateFinishedEvent>()
            // These systems should run after any app code that might mutate
            // CharAnimationState or Motion. And set_directions might have
            // mutated the animation state, so that should take effect before
            // the main animate system.
            .add_systems(
                Update,
                (
                    charanm_atlas_reassign_system,
                    charanm_set_directions_system,
                    charanm_animate_system,
                    charanm_update_colliders_system,
                )
                    .chain()
                    .in_set(CharAnimationSystems),
            )
            .configure_sets(Update, CharAnimationSystems.after(SpriteChangers));
    }
}

// Right, so, how do we animate these dudes? (and thus, what do we need in our state component/bundle?)
// Starting in media res:
// - Each step, tick down a TIMER (non-repeating) for the current frame.
// - When the timer runs out, switch your FRAME INDEX to the next frame (or, WRAP AROUND if you're configured to loop).

fn charanm_set_directions_system(
    mut query: Query<(&mut CharAnimationState, &Motion)>,
    animations: Res<Assets<CharAnimation>>,
) {
    for (mut state, motion) in query.iter_mut() {
        if let Some(animation) = animations.get(&state.animation) {
            // Combine facing + animation's directionality to decide.
            let (dir, flip_x) = match animation.directionality {
                Directionality::Zero => (Dir::Neutral, false),
                Directionality::OneE => {
                    // Variant always E, but flip sprite if they turn west. Actually this is a bit
                    // more subtle, bc if they didn't just TURN in a horizontal direction (i.e. they
                    // were going W but then turned due north), we want to preserve PRIOR flip.
                    // BTW, I can't decide yet whether Four directionality would also have this problem
                    // when downgrading from an Eight sprite.
                    let prior_flip = state.flip_x;
                    let flip = match Dir::ordinal_from_angle(motion.facing) {
                        Dir::E | Dir::NE | Dir::SE => false,
                        Dir::W | Dir::NW | Dir::SW => true,
                        _ => prior_flip,
                    };
                    // Alternately, you could match Dir::cardinal_from_angle and only react to E or W.
                    // But I think this should give snappier results with analog input.
                    (Dir::E, flip)
                },
                Directionality::Four => (Dir::cardinal_from_angle(motion.facing), false),
            };
            // set unconditionally, and let change_variant sort out whether to actually change anything.
            state.change_variant(dir);
            state.flip_x = flip_x;
        }
    }
}

/// This system handles the main logic of progressing through an animation:
/// tick the frame timer, then reset it and flip to the next animation frame
/// if it's time to do that.
pub fn charanm_animate_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<(
        &mut CharAnimationState,
        &mut Sprite,
        &mut TextureAtlas,
        Entity,
    )>,
    time: Res<Time>,
    mut finished_events: EventWriter<AnimateFinishedEvent>,
) {
    for (mut state, mut sprite, mut atlas, entity) in query.iter_mut() {
        let Some(animation) = animations.get(&state.animation) else {
            continue;
        };
        let Some(variant_name) = &state.variant else {
            continue;
        };
        // get the stugff
        let Some(variant) = animation.variants.get(variant_name) else {
            continue;
        };

        let mut updating_frame = false;

        // update the timer... or initialize it, if it's missing.
        if let Some(frame_timer) = &mut state.frame_timer {
            frame_timer.tick(time.delta());
            'timers: while state.timer_just_finished() {
                // Determine the next frame
                let frame_count = variant.frames.len();
                let next_frame = (state.frame + 1) % frame_count;

                // If next is 0, we just finished the *last* frame... fire an
                // event in case anyone wants to do something about that. This
                // is valid for single-frame animations too, although it might
                // not seem it at first blush.
                if next_frame == 0 {
                    finished_events.send(AnimateFinishedEvent(entity));
                    // If this is a non-looping animation, we bail now and leave
                    // it perma-stuck on the final frame. Its timer will keep
                    // accumulating, and this loop won't run again until the
                    // animation is changed.
                    match state.playback {
                        Playback::Once => {
                            break 'timers;
                        },
                        // nothing interesting yet for looping animations, but I
                        // want the exhaustiveness check from `match` just in case.
                        Playback::Loop => (),
                    }
                }

                updating_frame = true;
                let excess_time = state.frame_timer.as_ref().unwrap().countup_elapsed();

                // increment+loop frame, and replace the timer with the new frame's duration
                state.frame = next_frame;
                let duration = variant.resolved_frame_time(state.frame, state.frame_time_override);
                let mut new_timer = CountupTimer::new(duration);
                new_timer.tick(excess_time);
                state.frame_timer = Some(new_timer);
            }
        } else {
            // must be new here. initialize the timer w/ the current
            // frame's duration, can start ticking on the next loop.
            updating_frame = true;
            let duration = variant.resolved_frame_time(state.frame, state.frame_time_override);
            state.frame_timer = Some(CountupTimer::new(duration));
        }

        // ok, where was I.
        if updating_frame {
            // Uh, dig into the variant and frame to see what actual
            // texture index we oughtta use, and set it.
            let frame = &variant.frames[state.frame];
            // Atlas-based sprite data is split across the Sprite (shared with all
            // the stuff a non-animating sprite needs) and the TextureAtlas (just
            // the index of which rect to show, and a Handle<...Layout>).
            atlas.index = frame.index;
            sprite.flip_x = state.flip_x;
            // Also, set the origin:
            let anchor = if state.flip_x {
                // This is great: since the anchor format normalizes the texture space to
                // 1.0 x 1.0 regardless of texture dimensions, AND sets 0.0 to the center
                // of the texture, we can just... flip it.
                flip_vec2_x(frame.anchor)
            } else {
                frame.anchor
            };
            sprite.anchor = Anchor::Custom(anchor);
            // But leave colliders to their own systems.
        }
    }
}

/// The main animate system updates the origin because everything's gotta have
/// one, but maybe not everything needs a collider, even if its sprite has one.
/// So, we only update colliders for entities who have opted in by having one
/// added to them at some point.
/// Notes:
/// - We don't remove collider components! The component itself needs to be able
/// to express the condition of "yeah I do this type of interaction, but have no
/// collider info for this frame".
/// - We're filtering on `Changed<TextureAtlas>` because the animation state
/// contains a Timer and thus changes constantly... but we only update the atlas
/// index when it's time to flip frames.
fn charanm_update_colliders_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<
        (
            &CharAnimationState,
            &mut Walkbox,
            Option<&mut Hitbox>,
            Option<&mut Hurtbox>,
        ),
        Changed<TextureAtlas>,
    >,
) {
    for (state, mut walkbox, hitbox, hurtbox) in query.iter_mut() {
        let Some(animation) = animations.get(&state.animation) else {
            continue;
        };
        let Some(variant_name) = &state.variant else {
            continue;
        };
        let Some(variant) = animation.variants.get(variant_name) else {
            continue;
        };
        let frame = &variant.frames[state.frame];

        // If there's no walkbox in the frame, you get a 0-sized rectangle at your origin.
        let sprite_walkbox = frame.walkbox.unwrap_or_default();
        walkbox.0 = maybe_mirrored(sprite_walkbox, state.flip_x);

        // Hitbox is both optional as a whole (entity does/doesn't ever attack), and has
        // an optional inner value (entity is/isn't dealing damage this frame).
        if let Some(mut hit) = hitbox {
            hit.0 = frame.hitbox.map(|r| maybe_mirrored(r, state.flip_x));
        }
        // Same for hurtbox.
        if let Some(mut hurt) = hurtbox {
            hurt.0 = frame.hurtbox.map(|r| maybe_mirrored(r, state.flip_x));
        }
    }
}

// tiny util for maybe mirroring a rect.
fn maybe_mirrored(r: Rect, flip_x: bool) -> Rect {
    if flip_x {
        flip_rect_x(r)
    } else {
        r
    }
}

/// Texture atlas sprites require two asset Handles, per the SpriteSheetBundle:
///
/// - a Handle<Image> (as a loose component)
/// - a Handle<TextureAtlasLayout> (inside the TextureAtlas component)
///
/// For an entity managed by the char_animation system, both of those must always
/// match the handles from the CharAnimationState. So, this system syncs em, with
/// a quick check first to avoid spurious change events.
fn charanm_atlas_reassign_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<(&CharAnimationState, &mut TextureAtlas, &mut Handle<Image>)>,
) {
    for (state, mut atlas, mut texture) in query.iter_mut() {
        // get the animation, get the handle off it, compare the handles, and if
        // they don't match, replace the value.
        if let Some(animation) = animations.get(&state.animation) {
            if atlas.layout != animation.layout {
                atlas.layout = animation.layout.clone();
            }
            if *texture != animation.texture {
                *texture = animation.texture.clone();
            }
        }
    }
}
