//! Separate spatial components for entities that participate in collision
//! physics.
//!
//! OK, so here's the scoop: GlobalTransform is awkward or prohibitive to work
//! with for physics purposes, even real dumb physics like I intend to do. It's
//! better to interact with an immediately responsive transform that doesn't
//! require an end-of-frame sync up the hierarchy. HOWEVER, because I'm using
//! bevy_ecs_ldtk to position tile-based walls and starting positions for
//! entities in my levels, I'm going to have entities all over the place that
//! are parented to something with a non-zero transform. So. In order to
//! interact with all these in a reliable shared reference frame, a couple
//! conditions need met: make sure physics objects's offsets from zero are
//! static (i.e. their hierarchical parents are NOT moving around; no
//! independently colliding objects being held in your paw), and make a note of
//! those offsets when they're spawned. As best as I can tell, LDTK tiles and
//! entities meet that static requirement -- tiles are children of their
//! tilemaps, and entities are either children of their level or of the world.
//!
//! The rules for messing with physical entities is this:
//!
//! - Only read or mutate their PhysTransforms, don't manipulate their
//!   Transforms.
//! - This module's systems sync PhysTransform to Transform every frame.
//! - To make an entity physical, just insert its offset; we'll handle the rest
//!   in this module.

use bevy::prelude::*;
use bevy_inspector_egui::Inspectable;

/// Global offset from 0,0 for entities that particpate in physical interactions.
#[derive(Component, Deref, DerefMut, Inspectable)]
pub struct PhysOffset(pub Vec2);

/// Isolated transform component for things that participate in physical
/// interactions. (We're not supporting rotation or scale, so it's just
/// translation for now.)
#[derive(Component, Inspectable)]
pub struct PhysTransform {
    pub translation: Vec3,
}

/// System: Add PhysTransform to entities that just received their PhysOffset.
pub fn add_new_phys_transforms(
    mut commands: Commands,
    offset_q: Query<(Entity, &PhysOffset, &Transform), (Added<PhysOffset>, Without<PhysTransform>)>,
) {
    for (entity, offset, transform) in offset_q.iter() {
        let phys_transform = PhysTransform {
            translation: transform.translation + offset.0.extend(0.0),
        };
        commands.entity(entity).insert(phys_transform);
    }
}

/// System: Sync PhysTransform to Transform at end of frame, before the
/// hierarchical GlobalTransform sync.
pub fn sync_phys_transforms(mut query: Query<(&PhysTransform, &PhysOffset, &mut Transform)>) {
    for (phys_transform, offset, mut transform) in query.iter_mut() {
        transform.translation = phys_transform.translation - offset.0.extend(0.0);
    }
}
