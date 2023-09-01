//! This is an as-cheap-as-possible replacement for bevy_spatial with the rstar
//! feature enabled, so I can do very basic spatial lookups without the extra dep.

use std::marker::PhantomData;

use bevy::ecs::query::WorldQuery;
use bevy::prelude::*;
use rstar::{DefaultParams, PointDistance, RTree, RTreeObject, AABB};

use crate::phys_space::PhysTransform;

/// A little Entity + position wrapper for storing in an r* tree. So the idea
/// here is, the rtree gets a nice well-defined type with legible impls of stuff
/// to hold onto, but the interface we expose to gameplay bevy systems is plain
/// Vec2s, Entities, and tuples of same.
pub struct EntityLoc {
    pub loc: Vec2,
    pub entity: Entity,
}

// Lil from/into helpers for easy construction
impl From<(Vec2, Entity)> for EntityLoc {
    fn from(value: (Vec2, Entity)) -> Self {
        EntityLoc {
            loc: value.0,
            entity: value.1,
        }
    }
}
impl From<(Entity, Vec2)> for EntityLoc {
    fn from(value: (Entity, Vec2)) -> Self {
        EntityLoc {
            loc: value.1,
            entity: value.0,
        }
    }
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
// Question: would anything be nicer if I implemented Point for Vec2?

/// Internal component which tracks the last position at which the entity was updated in the tree.
#[derive(Component)]
pub struct MovementTracked<T> {
    pub lastpos: Vec2,
    pub component_type: PhantomData<T>,
}

impl<T> MovementTracked<T> {
    pub fn new(last: Vec2) -> Self {
        MovementTracked {
            lastpos: last,
            component_type: PhantomData,
        }
    }
}

// Ok, so we need to be generic over the marker component, so I can have multiple instances.
pub struct RstarPlugin<MarkComp> {
    #[doc(hidden)]
    component_type: PhantomData<MarkComp>,
}

impl<MarkComp> RstarPlugin<MarkComp> {
    pub fn new() -> Self {
        Self {
            component_type: PhantomData,
        }
    }
}

// Need a plugin impl... fill this in later, bc it's the meat of it.
impl<MarkComp> Plugin for RstarPlugin<MarkComp>
where
    MarkComp: Component + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        let tree_access = RstarAccess::<MarkComp>::new();
        app.insert_resource(tree_access)
            .add_systems(PostStartup, add_added::<MarkComp>)
            .add_systems(
                PostUpdate,
                (
                    delete::<MarkComp>,
                    add_added::<MarkComp>,
                    update_moved::<MarkComp>,
                )
                    .chain(),
            );
    }
}

// Also, need clone/copy/default, come back to that cuz it's easy

// THEn we need the *resource* we're inserting, -- also generic over the marker.
// That resource is gonna need tree-item CRUD methods basically.
// I think let's start there!
// The ONE lookup method I'm using is within_distance.

/// A resource for doing spatial queries from the set of entities that have some
/// marker component on them.
#[derive(Resource)]
pub struct RstarAccess<MarkComp> {
    #[doc(hidden)]
    component_type: PhantomData<MarkComp>,
    /// The underlying RTree struct.
    pub tree: RTree<EntityLoc, DefaultParams>,
}

// These consts were members of the plugin in bevy_spatial, but I don't need to be generic like that.

// The amount of entities which moved per frame after which the tree is fully recreated instead of updated.
// Default from bevy_spatial: 100.
const RECREATE_AFTER: usize = 100;
// The distance after which a entity is updated in the tree
// Default from bevy_spatial: 1.0.
const MIN_MOVED: f32 = 1.0;
const MIN_MOVED_SQUARED: f32 = MIN_MOVED * MIN_MOVED; // powi() and powf() aren't const 😹

// Mostly lifted directly from bevy_spatial! (And mostly just delegating to the rstar crate.)
#[allow(dead_code)]
impl<MarkComp> RstarAccess<MarkComp> {
    fn new() -> Self {
        let tree: RTree<EntityLoc, DefaultParams> = RTree::new(); // don't need new_with_params
        Self {
            component_type: PhantomData,
            tree,
        }
    }

    /// Get the nearest neighbour to a position.
    pub fn nearest_neighbour(&self, loc: Vec2) -> Option<(Vec2, Entity)> {
        let res = self.tree.nearest_neighbor(&[loc.x, loc.y]);
        res.map(|point| (point.loc, point.entity))
    }

    /// Get the `k` neighbours to `loc`
    ///
    /// If `loc` is the location of a tracked entity, you might want to skip the first.
    pub fn k_nearest_neighbour(&self, loc: Vec2, k: usize) -> Vec<(Vec2, Entity)> {
        let _span = info_span!("k-nearest").entered();

        return self
            .tree
            .nearest_neighbor_iter(&[loc.x, loc.y])
            .take(k)
            .map(|e| (e.loc, e.entity))
            .collect::<Vec<(Vec2, Entity)>>();
    }

    /// Get all entities within a certain distance (radius) of `loc`
    pub fn within_distance(&self, loc: Vec2, distance: f32) -> Vec<(Vec2, Entity)> {
        let _span = info_span!("within-distance").entered();

        return self
            .tree
            .locate_within_distance([loc.x, loc.y], distance.powi(2))
            .map(|e| (e.loc, e.entity))
            .collect::<Vec<(Vec2, Entity)>>();
    }

    /// Recreates the tree with the provided entity locations/coordinates.
    ///
    /// Only use if manually updating, the plugin will overwrite changes.
    pub fn recreate(&mut self, all: Vec<(Vec2, Entity)>) {
        let _span_d = info_span!("collect-data").entered();
        let data: Vec<EntityLoc> = all.iter().map(|e| (*e).into()).collect();
        _span_d.exit();
        let _span = info_span!("recreate").entered();
        let tree: RTree<EntityLoc, DefaultParams> = RTree::bulk_load_with_params(data);
        self.tree = tree;
    }

    /// Adds a point to the tree.
    ///
    /// Only use if manually updating, the plugin will overwrite changes.
    pub fn add_point(&mut self, point: (Vec2, Entity)) {
        self.tree.insert(point.into())
    }

    /// Removes a point from the tree.
    ///
    /// Only use if manually updating, the plugin will overwrite changes.
    pub fn remove_point(&mut self, point: (Vec2, Entity)) -> bool {
        self.tree.remove(&point.into()).is_some()
    }

    /// Removed a point from the tree by its entity.
    ///
    /// Only use if manually updating, the plugin will overwrite changes.
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        let point = EntityLoc::from((entity, Vec2::ZERO));
        self.tree.remove(&point).is_some()
    }

    /// Size of the tree
    pub fn size(&self) -> usize {
        self.tree.size()
    }
}

// Then we're gonna need the systems -- add_added, delete, and update_moved.

fn add_added<MarkComp>(
    mut tree_access: ResMut<RstarAccess<MarkComp>>,
    mut commands: Commands,
    all_query: Query<(Entity, &PhysTransform), With<MarkComp>>,
    added_query: Query<(Entity, &PhysTransform), Added<MarkComp>>,
) where
    MarkComp: Component,
{
    let added: Vec<(Entity, &PhysTransform)> = added_query.iter().collect();
    // Why re-create if over-half-size? unsure yet
    if added.len() >= tree_access.size() / 2 {
        let recreate = info_span!("recreate_with_all", name = "recreate_with_all").entered();
        let all: Vec<(Vec2, Entity)> = all_query
            .iter()
            .map(|(entity, transform)| {
                let loc = transform.translation.truncate();
                // 1. add last-position component, while we're iterating anyway
                commands
                    .entity(entity)
                    .insert(MovementTracked::<MarkComp>::new(loc));
                // 0. finish map transform
                (loc, entity)
            })
            .collect();

        // 2. update tree
        tree_access.recreate(all);
        recreate.exit();
    } else {
        let update = info_span!("partial_update", name = "partial_update").entered();
        for (entity, transform) in added {
            let loc = transform.translation.truncate();
            // 1. update tree
            tree_access.add_point((loc, entity));
            // 2. add last-position component
            commands
                .entity(entity)
                .insert(MovementTracked::<MarkComp>::new(loc));
        }

        update.exit();
    }
}

// OK, this is really interesting, I hadn't seen a worldquery struct before
// taking spatial apart. Seems occasionally convenient!
#[derive(WorldQuery)]
#[world_query(mutable)]
pub struct TrackedQuery<'a, MarkComp>
where
    MarkComp: Sync + Send + 'static,
{
    pub entity: Entity,
    pub transform: &'a PhysTransform,
    pub tracker: &'static mut MovementTracked<MarkComp>,
}

fn update_moved<MarkComp>(
    mut tree_access: ResMut<RstarAccess<MarkComp>>,
    mut set: ParamSet<(
        Query<TrackedQuery<MarkComp>, Changed<PhysTransform>>,
        Query<TrackedQuery<MarkComp>>,
    )>,
) where
    MarkComp: Component,
{
    // decide what we're doing by checking how much movement happened, then
    // update tree and update trackers.
    // (entity, lastpos, currentpos)
    let move_dist = info_span!(
        "compute_moved_significant_distance",
        name = "compute_moved_significant_distance"
    )
    .entered();
    let moved: Vec<(Entity, Vec2, Vec2)> = set
        .p0()
        .iter()
        .filter_map(|tqi| {
            let entity = tqi.entity;
            let last = tqi.tracker.lastpos;
            let cur = tqi.transform.translation.truncate();
            if last.distance_squared(cur) >= MIN_MOVED_SQUARED {
                Some((entity, last, cur))
            } else {
                None
            }
        })
        .collect();
    move_dist.exit();

    // See, and unlike add_added, this compares to constant number instead of proportion of size 🤷🏽
    if moved.len() >= RECREATE_AFTER {
        let recreate = info_span!("recreate_with_all", name = "recreate_with_all").entered();
        let all: Vec<(Vec2, Entity)> = set
            .p1()
            .iter_mut()
            .map(|mut tqi| {
                let cur = tqi.transform.translation.truncate();
                // 1. update trackers
                tqi.tracker.lastpos = cur;
                // 0. finish map transform
                (cur, tqi.entity)
            })
            .collect();
        // 2. update tree
        tree_access.recreate(all);
        recreate.exit();
    } else {
        let update = info_span!("partial_update", name = "partial_update").entered();
        let mut p1 = set.p1();
        for (entity, last, cur) in moved {
            let Ok(mut mut_tqi) = p1.get_mut(entity) else {
                continue;
            };
            // Hmm, conditional guard on point already being there... I think
            // that only finds by entity, bc of EntityPos's PartialEq.
            if tree_access.remove_point((last, entity)) {
                // 1. update tree
                tree_access.add_point((cur, entity));
                // 2. update trackers
                mut_tqi.tracker.lastpos = cur;
            }
        }
        update.exit();
    }
}

fn delete<MarkComp>(
    mut tree_access: ResMut<RstarAccess<MarkComp>>,
    mut removed: RemovedComponents<MarkComp>,
) where
    MarkComp: Component,
{
    for entity in removed.iter() {
        tree_access.remove_entity(entity);
    }
}
