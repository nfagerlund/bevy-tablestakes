//! This is an as-cheap-as-possible replacement for bevy_spatial with the rstar
//! feature enabled, so I can do very basic spatial lookups without the extra dep.

use std::marker::PhantomData;

use bevy::prelude::*;
use rstar::{DefaultParams, RTree, RTreeObject, RTreeParams, AABB};

pub struct EntityLoc {
    pub loc: Vec2,
    pub entity: Entity,
}

// Like in bevy_spatial, we're only comparing entity identity.
impl PartialEq for EntityLoc {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

// Need RTreeObject impl to insert these into an rtree.
impl RTreeObject for EntityLoc {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.loc.into())
    }
}

// Ok, so we need to be generic over the marker component, so I can have multiple instances.
struct RstarPlugin<MarkComp> {
    #[doc(hidden)]
    component_type: PhantomData<MarkComp>,
}

// Need a plugin impl... fill this in later, bc it's the meat of it.
impl<Marker> Plugin for RstarPlugin<Marker>
where
    Marker: Component + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {}
}

// Also, need clone/copy/default, come back to that cuz it's easy

// THEn we need the *resource* we're inserting, -- also generic over the marker.
// That resource is gonna need tree-item CRUD methods basically.
// I think let's start there!
// The ONE lookup method I'm using is within_distance.

#[derive(Resource)]
struct RstarAccess<MarkComp> {
    #[doc(hidden)]
    component_type: PhantomData<MarkComp>,
    /// The underlying RTree struct.
    pub tree: RTree<EntityLoc, DefaultParams>,
    /// The amount of entities which moved per frame after which the tree is fully recreated instead of updated.
    pub recreate_after: usize,
    /// The distance after which a entity is updated in the tree
    pub min_moved: f32,
}

// Then we're gonna need the systems -- add_added, delete, and update_moved.
