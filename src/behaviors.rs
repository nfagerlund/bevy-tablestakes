//! Behavioral components and events for... all kinds of shit.

use crate::{
    debug_settings::NumbersSettings, // Mmmmmmm need to reconsider where this goes
    input::CurrentInputs,
    movement::{Motion, Speed},
    phys_space::PhysTransform,
    Player,
};
use bevy::prelude::*;

/// A Bundle-implementing type representing all behaviors. Useful for removing behaviors when resetting everything.
pub type AllBehaviors = (
    AggroRange,
    Headlong,
    Hitstun,
    Knockback,
    Launch,
    MobileFree,
    MobileFixed,
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

/// Behavior: interested in finding a player to hunt, within a given distance.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AggroRange(pub f32);

/// Behavior: currently hunting a player
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Aggro {
    pub target: Entity,
    /// The entity's home point, and the max distance it's willing to stray from it.
    pub limit: Option<(Vec2, f32)>,
}

// ------- Behavior events -------

pub struct BehaviorEventsPlugin;
impl Plugin for BehaviorEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Rebound>().add_event::<AggroActivate>();
    }
}

/// Event: got your ass bounced off something
#[derive(Event)]
pub struct Rebound {
    pub entity: Entity,
    pub vector: Vec2,
}

/// Event: GET IM
#[derive(Event)]
pub struct AggroActivate {
    pub subject: Entity,
    pub target: Entity,
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

/// Plan motion toward an entity. TODO: aggro is just a special case of this,
/// so let's generalize it.
pub fn mobile_chase_entity(
    mut chase_q: Query<(&mut Motion, &Aggro, &Speed, &PhysTransform)>,
    all_locs_q: Query<&PhysTransform>,
) {
    chase_q.for_each_mut(|(mut motion, aggro, speed, transform)| {
        if let Ok(target_transform) = all_locs_q.get(aggro.target) {
            let difference = target_transform.translation - transform.translation;
            let input = difference.truncate().normalize_or_zero();
            motion.velocity += input * speed.0;
            motion.face(input);
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

/// Aggro onto player if you spot one
pub fn acquire_aggro(
    player_q: Query<(Entity, &PhysTransform), With<Player>>,
    enemy_q: Query<(Entity, &PhysTransform, &AggroRange), Without<Player>>,
    mut activate: EventWriter<AggroActivate>,
) {
    // ....... hmm, spatial query, or just skip it?
    for (enemy, e_transform, range) in enemy_q.iter() {
        let e_loc = e_transform.translation.truncate();
        for (player, p_transform) in player_q.iter() {
            if e_loc.distance(p_transform.translation.truncate()) <= range.0 {
                activate.send(AggroActivate {
                    subject: enemy,
                    target: player,
                });
            }
        }
    }
}
