use bevy::prelude::*;
use bevy_inspector_egui::Inspectable;

/// Global offset from 0,0 for entities that particpate in collision physics.
/// OK, so here's the scoop: GlobalTransform is awkward or prohibitive to work
/// with for physics purposes, even real dumb physics like I intend to do. It's
/// better to interact with an immediately responsive transform that doesn't
/// require an end-of-frame sync up the hierarchy. HOWEVER, because I'm using
/// bevy_ecs_ldtk to position tile-based walls and starting positions for
/// entities in my levels, I'm going to have entities all over the place that
/// are parented to something with a non-zero transform. So. In order to
/// interact with all these in a reliable shared reference frame, a couple
/// conditions need met: make sure physics objects's offsets from zero are
/// static (i.e. their hierarchical parents are NOT moving around), and make a
/// note of them when they're spawned. As best as I can tell, LDTK tiles and
/// entities meet that static requirement -- tiles are children of their
/// tilemaps, and entities are either children of their level or of the world.
/// Oh, also, I think for broad phase I can probably get away with cheating with
/// GlobalTransform, as long as I make my margin of error large enough that you
/// can't get away from it in a single simulation frame... at least for now it
/// should be fine. To be more rigorous I should probably roll my own spatial
/// query thing and use physics space instead.
#[derive(Component, Deref, DerefMut, Inspectable)]
pub struct PhysOffset(pub Vec2);

/// Isolated transform component for things that participate in physical
/// interactions. (We're not supporting rotation or scale, so it's just
/// translation for now.)
#[derive(Component, Inspectable)]
pub struct PhysTransform {
    pub translation: Vec3,
}
