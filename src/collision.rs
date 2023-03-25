use std::collections::HashMap;

use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};
use bevy_ecs_ldtk::prelude::*;
use bevy_inspector_egui::Inspectable;

/// BBox defining the space an entity takes up on the ground.
#[derive(Component, Inspectable, Default)]
pub struct Walkbox(pub Rect);

/// BBox defining the space where an entity can be hit by attacks.
#[derive(Component, Inspectable, Default)]
pub struct Hitbox(pub Rect);
// ...and then eventually I'll want Hurtbox for attacks, but, tbh I have no
// idea how to best handle that yet. Is that even a component? Or is it a larger
// data structure associated with an animation or something?

pub fn centered_rect(width: f32, height: f32) -> Rect {
    let min = Vec2::new(-width / 2., -height / 2.);
    let max = Vec2::new(width / 2., height / 2.);
    Rect { min, max }
}

pub fn _bottom_centered_rect(width: f32, height: f32) -> Rect {
    let min = Vec2::new(-width / 2., 0.);
    let max = Vec2::new(width / 2., height);
    Rect { min, max }
}

pub struct Collision {
    pub location: Vec2,
    pub normal: Vec2,
    pub relative_time: f32,
}

/// Detects collision between a ray and a rect. Algorithm via https://www.youtube.com/watch?v=8JJ-4JgR7Dg
pub fn ray_vs_rect(
    ray_pos: Vec2,
    ray_move: Vec2,
    rect_min: Vec2,
    rect_max: Vec2,
) -> Option<Collision> {
    // First, find the four points where the ray contacts the rect's axes,
    // expressed in terms of fractions of the proposed movement. Note that these
    // might be infinite or might be large multiples of the movement, but we'll
    // ultimately only be interested in the ones that are between (0.0, 1.0].
    let a = (rect_min - ray_pos) / ray_move;
    let b = (rect_max - ray_pos) / ray_move;
    let near_x_time = f32::min(a.x, b.x);
    let near_y_time = f32::min(a.y, b.y);
    let far_x_time = f32::max(a.x, b.x);
    let far_y_time = f32::max(a.y, b.y);
    // ^^ This is specifically pretty unoptimized while I decide wtf I'm doing
    // here. I think this automatically handles infinite (divide by 0.0) values
    // on one axis, which is what happens if you're moving horizontally or
    // vertically.

    // The ray's line only actually intersects the rectangle if near_x is <=
    // far_y, and near_y is <= far_x.
    if near_x_time > far_y_time || near_y_time > far_x_time {
        return None;
    }

    // A ray enters the rectangle when both near axes have been crossed, and
    // leaves it when at least one far axis has been crossed.
    let near_hit_time = near_x_time.max(near_y_time);
    let far_hit_time = far_x_time.min(far_y_time);

    // The RAY only actually intersects the rectangle if it's not pointed away
    // from it.
    if far_hit_time < 0.0 {
        return None;
    }

    None
    // OK, so the ray actually intersects it! First, where at?

    // Then, which side did we enter from?
}

/// An AABB that's located in absolute space, probably produced by combining a
/// BBox with an origin offset.
#[derive(Copy, Clone, Debug)]
pub struct AbsBBox {
    pub min: Vec2,
    pub max: Vec2,
}

impl AbsBBox {
    /// Locate a rect in space, given an origin point
    pub fn from_rect(rect: Rect, origin: Vec2) -> Self {
        Self {
            min: rect.min + origin,
            max: rect.max + origin,
        }
    }

    pub fn ray_collide(&self, ray_start: Vec2, ray_motion: Vec2) -> Option<Collision> {
        // First, we find the "relative times" where the line defined by the ray
        // intersects the four lines defined by the sides of the rectangle. This
        // might not actually result in the "ray" intersecting the "rectangle"
        // at all, and in cases where there's no motion on an axis it'll be an
        // infinity of some kind.
        let inverse_motion = 1.0 / ray_motion;
        let Vec2 {
            x: left_at,
            y: bottom_at,
        } = (self.min - ray_start) * inverse_motion;
        let Vec2 {
            x: right_at,
            y: top_at,
        } = (self.max - ray_start) * inverse_motion;
        // And this is the point where I diverge from guy: I want to throw away
        // as little precision as I can, so, preserve the intermediate knowledge
        // here instead of reconstructing it later.
        let (near_x_at, far_x_at) = if left_at < right_at {
            (left_at, right_at)
        } else {
            (right_at, left_at)
        };
        let (near_y_at, far_y_at) = if bottom_at < top_at {
            (bottom_at, top_at)
        } else {
            (top_at, bottom_at)
        };

        // Early return: line never intersects the rectangle.
        if near_x_at > far_y_at || near_y_at > far_x_at {
            return None;
        }
    }

    /// Check whether an absolutely positioned bbox overlaps with another one.
    pub fn collide(&self, other: Self) -> bool {
        self.overlaps_x(other) && self.overlaps_y(other)
    }

    /// Check whether two boxes overlap on the X axis.
    pub fn overlaps_x(&self, other: Self) -> bool {
        ! // neither
        (
            // right of other,
            self.min.x > other.max.x
            || // nor
            // left of other
            self.max.x < other.min.x
        )
    }

    /// Check whether two boxes overlap on the Y axis.
    pub fn overlaps_y(&self, other: Self) -> bool {
        ! // neither
        (
            // below other,
            self.max.y < other.min.y
            || // nor
            // above other
            self.min.y > other.max.y
        )
    }

    /// Return the new AbsBBox that would result from moving self by `movement`.
    pub fn translate(&self, movement: Vec2) -> Self {
        Self {
            min: self.min + movement,
            max: self.max + movement,
        }
    }

    /// Clamp another AbsBBox's proposed movement vector to prevent it from
    /// overlapping with this box. Vulnerable to tunnelling, but I could rewrite
    /// it to not be if I need to later. Don't feed this any NaNs.
    pub fn faceplant(&self, other: Self, mvt: Vec2) -> Vec2 {
        // If we have nothing to do with each other, done.
        if !self.collide(other.translate(mvt)) || mvt.length() == 0.0 {
            // easy peasy
            return mvt;
        }

        // If we're *already* entangled, gently suggest pushing out in the
        // opposite direction. ...but I'm not sure yet how I want to implement
        // that, so for now just bitch about it.
        if self.collide(other) {
            info!(
                "UH, FOR SOME REASON YOU ({other:?}) ARE STUCK IN THING ({self:?}). Consider leaving??"
            );
            return mvt;
        }

        // What's left? We're not already entangled, but the proposed move would
        // overlap us and we'd rather it not. Determine the minimal clamping to
        // the proposal that would keep us excluded.

        let mut res = mvt;

        // Check what happens if only moving X component
        if self.collide(other.translate(Vec2::new(res.x, 0.0))) {
            let x_dir = Toward::from_f32(mvt.x);
            res.x = match x_dir {
                // leftbound: negative, so + shrinks magnitude and max is smallest magnitude.
                Toward::Min => (self.max.x - other.min.x + 1.0).max(mvt.x),
                // rightbound: positive, so - shrinks magnitude and min is smallest magnitude.
                Toward::Max => (self.min.x - other.max.x - 1.0).min(mvt.x),
                Toward::Static => 0.0f32,
            };
        }

        // Then check what the Y move does after the X is a done deal
        if self.collide(other.translate(Vec2::new(res.x, res.y))) {
            let y_dir = Toward::from_f32(mvt.y);
            res.y = match y_dir {
                // downbound: negative, so + shrinks magnitude and max is smallest magnitude.
                Toward::Min => (self.max.y - other.min.y + 1.0).max(mvt.y),
                // upbound: positive, so - shrinks magnitude and min is smallest magnitude.
                Toward::Max => (self.min.y - other.max.y - 1.0).min(mvt.y),
                Toward::Static => 0.0f32,
            };
        }

        res
    }
}

/// Private helper enum for making some faceplant code more legible
enum Toward {
    Min,
    Max,
    Static,
}

impl Toward {
    fn from_f32(v: f32) -> Self {
        let sign = v.signum();
        if sign == 1.0 {
            Self::Max
        } else if sign == -1.0 {
            Self::Min
        } else {
            Self::Static
        }
    }
}

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
pub struct PhysicsSpaceOffset(pub Vec2);

/// Collidable solid marker component... but you also need a position Vec3 and a
/// size Vec2 from somewhere.
#[derive(Component)]
pub struct Solid;

/// Wall bundle for tilemap walls
#[derive(Bundle)]
pub struct Wall {
    solid: Solid,
    walkbox: Walkbox,
    offset: PhysicsSpaceOffset,
    int_grid_cell: IntGridCell,
    // transform: Transform, // This is needed, but it's handled by the plugin.
}

// Custom impl instead of derive bc... you'll see!
impl LdtkIntCell for Wall {
    fn bundle_int_cell(int_grid_cell: IntGridCell, layer_instance: &LayerInstance) -> Self {
        // there!! v. proud of finding this, the example just cheated w/ prior knowledge.
        let grid_size = layer_instance.grid_size as f32;
        let translation_offset = Vec2::new(
            grid_size / 2.0 + layer_instance.px_total_offset_x as f32,
            grid_size / 2.0 + layer_instance.px_total_offset_y as f32,
        );
        Wall {
            solid: Solid,
            // the plugin puts tile anchor points in the center:
            walkbox: Walkbox(centered_rect(grid_size, grid_size)),
            offset: PhysicsSpaceOffset(translation_offset),
            int_grid_cell,
        }
    }
}

// -- COLLIDER DEBUG MESH STUFF --

#[derive(Resource, Deref, DerefMut, Default)]
pub struct DebugAssets(HashMap<String, HandleUntyped>);

#[derive(Bundle, Default)]
pub struct WalkboxDebugBundle {
    pub mesh_bundle: MaterialMesh2dBundle<ColorMaterial>,
    pub marker: WalkboxDebug,
}
/// Marker component for walkbox debug mesh
#[derive(Component, Default)]
pub struct WalkboxDebug;
#[derive(Resource, Inspectable, Default)]
pub struct DebugColliders {
    show_walkboxes: bool,
}

// TODO: Maybe just convert this to a local resource for the debug spawner and
// use .entry().or() to do the initialization.
pub fn setup_debug_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut debug_assets: ResMut<DebugAssets>,
) {
    let walkbox_mesh = meshes.add(Mesh::from(shape::Quad::default()));
    let walkbox_material = materials.add(ColorMaterial::from(Color::rgba(0.5, 0.0, 0.5, 0.6)));
    debug_assets.insert("walkbox_mesh".to_string(), walkbox_mesh.clone_untyped());
    debug_assets.insert(
        "walkbox_material".to_string(),
        walkbox_material.clone_untyped(),
    );
}

/// Add debug mesh children to newly added collidable entities, so I can see
/// where their boundaries are. (Toggle visibility with inspector).
pub fn spawn_collider_debugs(
    // vv Added<Player> is a hack, but rn that's the only mobile collider so shrug
    new_collider_q: Query<Entity, Or<(Added<Solid>, Added<crate::Player>)>>,
    mut commands: Commands,
    debug_assets: Res<DebugAssets>,
) {
    for collider in new_collider_q.iter() {
        let (Some(mesh_untyped), Some(material_untyped)) = (debug_assets.get("walkbox_mesh"), debug_assets.get("walkbox_material"))
        else {
            warn!("Hey!! I couldn't get ahold of the collider debug assets for some reason!!");
            return;
        };
        let material = material_untyped.clone().typed::<ColorMaterial>();
        let mesh = Mesh2dHandle(mesh_untyped.clone().typed::<Mesh>());

        // info!("attaching debug mesh to {:?}", &collider);
        commands.entity(collider).with_children(|parent| {
            parent.spawn(WalkboxDebugBundle {
                mesh_bundle: MaterialMesh2dBundle {
                    mesh,
                    material,
                    visibility: Visibility { is_visible: false },
                    ..default()
                },
                marker: WalkboxDebug,
            });
        });
    }
}

/// Update size and position of collider debug meshes, since walkboxes can
/// change frame-by-frame.
pub fn debug_walkboxes_system(
    collider_q: Query<(Entity, &Walkbox)>,
    mut debug_mesh_q: Query<(&Parent, &mut Transform, &mut Visibility), With<WalkboxDebug>>,
    debug_settings: Res<DebugColliders>,
) {
    for (parent, mut transform, mut visibility) in debug_mesh_q.iter_mut() {
        let Ok((_, walkbox)) = collider_q.get(parent.get()) else {
            info!("?!?! tried to debug walkbox of some poor orphaned debug entity.");
            continue;
        };
        if debug_settings.show_walkboxes {
            visibility.is_visible = true;
            // ok... need to set our scale to the size of the walkbox, and then
            // offset our translation relative to our parent by the difference
            // between their walkbox's center and their actual anchor point
            // (because the mesh's anchor is always centered).
            let size = walkbox.0.max - walkbox.0.min;
            let center = walkbox.0.min + size / 2.0;
            // and of course, the anchor point of the rect is (0,0), by definition.
            transform.scale = size.extend(1.0);
            // draw on top of parent by A LOT. (cheating out of interactions with the TopDownMatter system.)
            transform.translation = center.extend(40.0);
        } else {
            visibility.is_visible = false;
            // we're done
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Rect;
    use bevy::prelude::Vec2;

    fn onesie_at_xy(x: f32, y: f32) -> AbsBBox {
        let rect = Rect {
            min: Vec2::new(0.0, 0.0),
            max: Vec2::new(1.0, 1.0),
        };
        let origin = Vec2::new(x, y);
        AbsBBox::from_rect(rect, origin)
    }

    #[test]
    fn absbbox_overlaps() {
        let reference_square = onesie_at_xy(0., 0.);

        assert!(!reference_square.collide(onesie_at_xy(1.5, 0.)));
        assert!(!reference_square.collide(onesie_at_xy(1.00001, 0.)));
        assert!(reference_square.collide(onesie_at_xy(0.8, 0.)));
        assert!(reference_square.collide(onesie_at_xy(1.0, 0.)));

        assert!(!reference_square.collide(onesie_at_xy(-1.5, 0.)));
        assert!(!reference_square.collide(onesie_at_xy(-1.00001, 0.)));
        assert!(reference_square.collide(onesie_at_xy(-0.8, 0.)));
        assert!(reference_square.collide(onesie_at_xy(-1.0, 0.)));

        assert!(!reference_square.collide(onesie_at_xy(0., 1.5)));
        assert!(!reference_square.collide(onesie_at_xy(0., 1.00001)));
        assert!(reference_square.collide(onesie_at_xy(0., 0.8)));
        assert!(reference_square.collide(onesie_at_xy(0., 1.0)));

        assert!(!reference_square.collide(onesie_at_xy(0., -1.5)));
        assert!(!reference_square.collide(onesie_at_xy(0., -1.00001)));
        assert!(reference_square.collide(onesie_at_xy(0., -0.8)));
        assert!(reference_square.collide(onesie_at_xy(0., -1.0)));
    }
}
