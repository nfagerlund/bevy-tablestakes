//! Behavioral components and events for... all kinds of shit.

use crate::{
    debug_settings::NumbersSettings, // Mmmmmmm need to reconsider where this goes
    input::CurrentInputs,
    movement::{Motion, Speed},
};
use bevy::prelude::*;

/// A Bundle-implementing type representing all behaviors. Useful for removing behaviors when resetting everything.
pub type AllBehaviors = (
    MobileFree,
    MobileFixed,
    Launch,
    Headlong,
    Hitstun,
    Knockback,
);

// ------- Behavior components -------

/// Behavior: able to move around in response to inputs. Only players do this.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct MobileFree;

/// Behavior: moving in a fixed direction, with a base input to be
/// multiplied by a speed and delta.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct MobileFixed {
    pub input: Vec2,
    pub face: bool,
}

/// Behavior: launched into the air but subject to gravity, not flying
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Launch {
    pub z_velocity: f32,
}

/// Behavior: moving too fast, and will rebound on wall hit.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Headlong;

/// Behavior: experiencing hitstun.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Hitstun;

/// Behavior: experiencing knockback.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Knockback;

// ------- Behavior events -------

/// Event: got your ass bounced off something
#[derive(Event)]
pub struct Rebound {
    pub entity: Entity,
    pub vector: Vec2,
}

// ------- Behavior systems -------

/// Plan motion for player when moving freely per inputs.
pub fn mobile_free_velocity(
    mut free_q: Query<(&mut Motion, &Speed), With<MobileFree>>,
    inputs: Res<CurrentInputs>,
) {
    free_q.for_each_mut(|(mut motion, speed)| {
        motion.velocity += inputs.movement * speed.0;
    });
}

/// Plan motion for entities moving on a fixed vector.
pub fn mobile_fixed_velocity(mut fixed_q: Query<(&mut Motion, &Speed, &MobileFixed)>) {
    fixed_q.for_each_mut(|(mut motion, speed, fixed)| {
        motion.velocity += fixed.input * speed.0;
        if fixed.face {
            motion.face(fixed.input);
        }
    });
}

pub const LAUNCH_GRAVITY: f32 = 255.0; // Reduce z-velocity by X per second. idk!

/// Plan vertical motion for entities that are launched (distinct from flying)
pub fn launch_and_fall(
    mut launched_q: Query<(&mut Motion, &mut Launch)>,
    time: Res<Time>,
    numbers: Res<NumbersSettings>,
) {
    let gravity = numbers.launch_gravity;
    launched_q.for_each_mut(|(mut motion, mut launch)| {
        motion.z_velocity += launch.z_velocity;
        launch.z_velocity -= gravity * time.delta_seconds();
    });
}
