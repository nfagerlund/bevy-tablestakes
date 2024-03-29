//! Systems and components for Actually Moving. This module disclaims responsibility for
//! PLANNING your movement! Right now that stuff's all in main. Anyway, there are
//! three implementations for movement at the moment, but the main one is
//! move_continuous_ray_test; it gives much better stability and feel.

use crate::{
    collision::{AbsBBox, Solid, Walkbox},
    phys_space::PhysTransform,
    space_lookup::RstarAccess,
};
use bevy::prelude::*;

type SolidsTree = RstarAccess<Solid>;
const SOLID_SCANNING_DISTANCE: f32 = 64.0;

/// Speed in pixels per second. This is used for _planning_ movement; once
/// the entity knows what it's trying to do this frame, it gets reduced to an
/// absolute velocity vector, in the Motion struct.
#[derive(Component, Reflect)]
pub struct Speed(pub f32);
impl Speed {
    pub const RUN: f32 = 60.0;
    pub const ROLL: f32 = 180.0;
    pub const BONK: f32 = 60.0;
    pub const ENEMY_RUN: f32 = 40.0;
}

/// Information about what the entity is doing, spatially speaking.
#[derive(Component, Reflect)]
pub struct Motion {
    /// The direction the entity is currently facing, in radians. Tracked
    /// separately because it persists even when no motion is planned.
    pub facing: f32,
    /// The linear velocity for this frame, as determined by the entity's state and inputs.
    pub velocity: Vec2,
    /// Linear velocity on the Z axis... very few things use this, so I'm keeping it out of
    /// the main velocity field.
    pub z_velocity: f32,
    /// ONLY used by the janky move_whole_pixel system, should probably go.
    pub remainder: Vec2,
    /// What happened in the move.
    pub result: Option<MotionResult>,
}
impl Motion {
    pub fn new(motion: Vec2) -> Self {
        let mut thing = Self {
            facing: 0.0, // facing east on the unit circle
            velocity: Vec2::ZERO,
            z_velocity: 0.0,
            remainder: Vec2::ZERO,
            result: None,
        };
        thing.face(motion);
        thing
    }

    pub fn face(&mut self, input: Vec2) {
        if input.length() > 0.0 {
            self.facing = Vec2::X.angle_between(input);
        }
    }
}

#[derive(Reflect)]
pub struct MotionResult {
    pub collided: bool,
    pub new_location: Vec2,
}

#[derive(Event)]
pub struct Landed(pub Entity);

/// Handle height motion... once I remove the other move systems, it should just get rolled into the remaining one.
pub(crate) fn move_z_axis(
    mut mover_q: Query<(Entity, &mut PhysTransform, &mut Motion)>,
    time: Res<Time>,
    mut landings: EventWriter<Landed>,
) {
    mover_q.for_each_mut(|(entity, mut transform, mut motion)| {
        // No collisions or anything, just move em.
        if motion.z_velocity != 0.0 {
            let mut new_z = transform.translation.z + motion.z_velocity * time.delta_seconds();
            motion.z_velocity = 0.0;
            if new_z <= 0.0 && transform.translation.z > 0.0 {
                // 1. Don't sink below the floor
                new_z = 0.0;
                // 2. Announce we're coming in hot
                landings.send(Landed(entity));
            }
            transform.translation.z = new_z;
        }
    });
}

pub(crate) fn move_continuous_no_collision(
    mut mover_q: Query<(&mut PhysTransform, &mut Motion)>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();
    for (mut transform, mut motion) in mover_q.iter_mut() {
        let raw_movement_intent = motion.velocity * delta;
        // then....... just do it!!
        transform.translation += raw_movement_intent.extend(0.0);
        motion.velocity = Vec2::ZERO;
        motion.result = Some(MotionResult {
            collided: false,
            new_location: transform.translation.truncate(),
        });
    }
}

pub(crate) fn move_continuous_ray_test(
    mut mover_q: Query<(&mut PhysTransform, &mut Motion, &Walkbox), Without<Solid>>,
    solids_q: Query<(&Walkbox, &PhysTransform), With<Solid>>,
    solids_tree: Res<SolidsTree>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();

    for (mut transform, mut motion, walkbox) in mover_q.iter_mut() {
        let planned_move = motion.velocity * delta;
        motion.velocity = Vec2::ZERO;
        let mut collided = false;
        // DON't bother creating an AbsBBox for player at this time. we'll use
        // their position and walkbox separately.
        if planned_move.length() == 0.0 {
            motion.result = None;
            continue;
            // yeah still don't like these motion struct semantics. later!
        }

        let player_loc = transform.translation.truncate();

        // search for nearby solids
        let candidate_solid_locs =
            solids_tree.within_distance(transform.translation.truncate(), SOLID_SCANNING_DISTANCE);
        let mut collided_solids: Vec<(AbsBBox, f32)> = candidate_solid_locs
            .iter()
            .filter_map(|&(_loc, ent)| {
                // UNWRAP: is ok as long as tree doesn't have stale entities.
                let (s_walkbox, s_transform) = solids_q.get(ent).unwrap();
                let s_origin = s_transform.translation.truncate();
                let solid = AbsBBox::from_rect(s_walkbox.0, s_origin);
                // Extend the solid's bounds by the opposite spans of the
                // player's walkbox, so a simple ray test will detect projected
                // collisions.
                let expanded_solid = solid.expand_for_ray_test(&walkbox.0);
                // Then, ray-cast test for collision, discard any we miss, and
                // keep the normalized time and the expanded box for re-use --
                // have to do each test twice, because you might collide a
                // further rectangle at a different position after correcting
                // for a nearer one.
                expanded_solid
                    .ray_collide(player_loc, planned_move)
                    .map(|c| (expanded_solid, c.normalized_time))
            })
            .collect();

        // ALAS, the intermediate .collect() is mandatory because sort takes slice, not iterator.
        collided_solids.sort_by(|a, b| a.1.total_cmp(&b.1));

        // ok, NOW we can actually resolve collisions. Second pass!
        let corrected_movement =
            collided_solids
                .iter()
                .fold(planned_move, |current_move, (expanded_solid, _)| {
                    // Always gotta return something outta this fold, so we'll mutate if still colliding.
                    let mut next_move = current_move;
                    if let Some(collision) =
                        expanded_solid.segment_collide(player_loc, current_move)
                    {
                        // HEY, here's where we mark collision for the result:
                        collided = true;

                        // Ok moving on
                        let vel_penalty = (1.0 - collision.normalized_time)
                            * collision.normal
                            * current_move.abs();
                        next_move = current_move + vel_penalty;
                    }
                    next_move
                });

        // All right, finally! Now just do it:
        transform.translation += corrected_movement.extend(0.0);
        motion.result = Some(MotionResult {
            collided,
            new_location: transform.translation.truncate(),
        });
    }
}

/// This version is willing to move by fractional pixels, and ignores movement.remainder.
pub(crate) fn move_continuous_faceplant(
    mut mover_q: Query<(&mut PhysTransform, &mut Motion, &Walkbox), Without<Solid>>,
    solids_q: Query<(&Walkbox, &PhysTransform), With<Solid>>,
    solids_tree: Res<SolidsTree>,
    time: Res<Time>,
) {
    // Make some assumptions: solid colliders are generally tiles, and tiles are
    // 16x16. Player walkbox is even smaller. We aren't moving more than, say,
    // two tile-widths per physics tick (and even that's outrageous). A radius
    // of 64 should be MORE than enough to sweep up everything. Reconsider if
    // assumptions change. We'll need to do the collection of candidate solids
    // *per-player-entity,* instead of outside the loop.

    let collect_sorted_solids =
        |player_loc: Vec2, mut candidate_locs: Vec<(Vec2, Entity)>| -> Vec<AbsBBox> {
            // Claiming ownership of that input vec bc I'm sorting.
            candidate_locs.sort_by(|a, b| {
                let a_dist = player_loc.distance_squared(a.0);
                let b_dist = player_loc.distance_squared(b.0);
                a_dist.total_cmp(&b_dist)
            });
            candidate_locs
                .iter()
                .map(|ent_loc| {
                    // unwrap is ok as long as tree doesn't have stale entities.
                    let (walkbox, transform) = solids_q.get(ent_loc.1).unwrap();
                    let origin = transform.translation.truncate();
                    AbsBBox::from_rect(walkbox.0, origin)
                })
                .collect()
        };

    let delta = time.delta_seconds();

    for (mut transform, mut motion, walkbox) in mover_q.iter_mut() {
        let mut planned_move = motion.velocity * delta;
        motion.velocity = Vec2::ZERO;
        let mut collided = false;
        let abs_walkbox = AbsBBox::from_rect(walkbox.0, transform.translation.truncate());

        if planned_move.length() == 0.0 {
            motion.result = None; // idk about keeping this semantics tho. awkward.
            continue;
        }

        // search for nearby solids
        let candidate_solid_locs =
            solids_tree.within_distance(transform.translation.truncate(), SOLID_SCANNING_DISTANCE);
        let solids = collect_sorted_solids(transform.translation.truncate(), candidate_solid_locs);

        // check for collisions and clamp the movement plan if we hit something
        for solid in solids.iter() {
            let clamped = solid.faceplant(abs_walkbox, planned_move);
            if clamped != planned_move {
                collided = true;
                planned_move = clamped;
            }
        }

        // commit it
        transform.translation += planned_move.extend(0.0);
        motion.result = Some(MotionResult {
            collided,
            new_location: transform.translation.truncate(),
        });
    }
}

/// Shared system for Moving Crap Around. Consumes a planned movement from
/// Motion component, updates direction on same as needed, writes result to...
pub(crate) fn move_whole_pixel(
    mut mover_q: Query<(&mut PhysTransform, &mut Motion, &Walkbox), Without<Solid>>,
    solids_q: Query<(&PhysTransform, &Walkbox), With<Solid>>,
    time: Res<Time>,
) {
    let solids: Vec<AbsBBox> = solids_q
        .iter()
        .map(|(transform, walkbox)| {
            // TODO: This can't handle solids that move, because GlobalTransform
            // lags by one frame. I don't have a solution for this yet! Treat them
            // separately? Manually sync frames of reference for everything?
            // Aggressively early-update the GlobalTransform of anything mobile
            // right after it moves? Anyway, for immobile walls we need to do *this*
            // because as of bevy_ecs_ldtk 0.5 / bevy_ecs_tilemap 0.8+, tile
            // entities are offset from (0,0) by a half a tile in both axes in order
            // to make the bottom left corner of the first tile render at (0,0).
            let origin = transform.translation.truncate();
            AbsBBox::from_rect(walkbox.0, origin)
        })
        .collect();
    let delta = time.delta_seconds();

    for (mut transform, mut motion, walkbox) in mover_q.iter_mut() {
        let raw_movement_intent = motion.velocity * delta;
        motion.velocity = Vec2::ZERO; // TODO should probably have this be an Option -> .take()

        // If we're not moving, stop running and bail.
        if raw_movement_intent.length() == 0.0 {
            // Don't hold onto sub-pixel remainders from previous move sequences once we stop
            motion.remainder = Vec2::ZERO;
            // No result
            motion.result = None;
            // Direction unchanged.
        } else {
            // Ok go for it!!

            let mut location = transform.translation.truncate();
            let mut collided = false;
            // Bring in any remainder
            let movement_intent = raw_movement_intent + motion.remainder;
            let move_pixels = movement_intent.round();
            let remainder = movement_intent - move_pixels;

            let mut move_x = move_pixels.x;
            let sign_x = move_x.signum();
            while move_x != 0. {
                let next_location = location + Vec2::new(sign_x, 0.0);
                let next_box = AbsBBox::from_rect(walkbox.0, next_location);
                if solids.iter().any(|s| s.collide(next_box)) {
                    // Hit a wall
                    collided = true;
                    break;
                } else {
                    location.x += sign_x;
                    move_x -= sign_x;
                }
            }
            let mut move_y = move_pixels.y;
            let sign_y = move_y.signum();
            while move_y != 0. {
                let next_origin = location + Vec2::new(0.0, sign_y);
                let next_box = AbsBBox::from_rect(walkbox.0, next_origin);
                if solids.iter().any(|s| s.collide(next_box)) {
                    // Hit a wall
                    collided = true;
                    break;
                } else {
                    location.y += sign_y;
                    move_y -= sign_y;
                }
            }

            // Commit it
            transform.translation.x = location.x;
            transform.translation.y = location.y;
            motion.remainder = remainder;
            motion.result = Some(MotionResult {
                collided,
                new_location: location,
            });
        }
    }
}
