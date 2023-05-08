//! This is an as-cheap-as-possible replacement for bevy_spatial with the rstar
//! feature enabled, so I can do very basic spatial lookups without the extra dep.

use std::marker::PhantomData;

use bevy::prelude::*;
use rstar::{DefaultParams, PointDistance, RTree, RTreeObject, RTreeParams, AABB};

/// A little Entity + position wrapper for storing in an r* tree.
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

// Need PointDistance impl to query these out of an rtree.
impl PointDistance for EntityLoc {
    fn distance_2(&self, point: &[f32; 2]) -> f32 {
        Vec2::from_slice(point).distance_squared(self.loc)
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

/// A resource for doing spatial queries from the set of entities that have some
/// marker component on them.
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

// Mostly lifted directly from bevy_spatial! (And mostly just delegating to the rstar crate.)
impl<MarkComp> RstarAccess<MarkComp> {
    /// Get the nearest neighbour to a position.
    fn nearest_neighbour(&self, loc: Vec2) -> Option<(Vec2, Entity)> {
        let res = self.tree.nearest_neighbor(&[loc.x, loc.y]);
        res.map(|point| (point.loc, point.entity))
    }

    /// Get the `k` neighbours to `loc`
    ///
    /// If `loc` is the location of a tracked entity, you might want to skip the first.
    fn k_nearest_neighbour(&self, loc: Vec2, k: usize) -> Vec<(Vec2, Entity)> {
        let _span = info_span!("k-nearest").entered();

        return self
            .tree
            .nearest_neighbor_iter(&[loc.x, loc.y])
            .take(k)
            .map(|e| (e.loc, e.entity))
            .collect::<Vec<(Vec2, Entity)>>();
    }

    /// Get all entities within a certain distance (radius) of `loc`
    fn within_distance(&self, loc: Vec2, distance: f32) -> Vec<(Vec2, Entity)> {
        let _span = info_span!("within-distance").entered();

        return self
            .tree
            .locate_within_distance([loc.x, loc.y], distance.powi(2))
            .map(|e| (e.loc, e.entity))
            .collect::<Vec<(Vec2, Entity)>>();
    }
}

// Then we're gonna need the systems -- add_added, delete, and update_moved.
