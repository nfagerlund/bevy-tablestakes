use std::collections::HashMap;

use crate::{phys_space::PhysOffset, DebugSettings};
use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};
use bevy_ecs_ldtk::prelude::*;

/// BBox defining the space an entity takes up on the ground.
#[derive(Component, Reflect, Default)]
pub struct Walkbox(pub Rect);

/// BBox defining the space where an entity can deal damage to others.
#[derive(Component, Reflect, Default)]
pub struct Hitbox(pub Option<Rect>);

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

#[derive(Debug)]
pub struct Collision {
    pub contact_point: Vec2,
    pub normal: Vec2,
    pub normalized_time: f32,
}

enum Side {
    Top,
    Bottom,
    Left,
    Right,
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

    /// Learned this algorithm from https://www.youtube.com/watch?v=8JJ-4JgR7Dg
    pub fn ray_collide(&self, ray_start: Vec2, ray_displacement: Vec2) -> Option<Collision> {
        // First, we find the "normalized times" where the LINE defined by the
        // ray intersects the four LINES defined by the sides of the rectangle.
        // There's always an answer for these intersections, but they might not
        // actually result in the ray intersecting the rectangle at all, and in
        // cases where there's no motion on an axis (e.g. straightly horizontal)
        // it'll be an infinity of some kind.
        let divide_by_ray_displacement = 1.0 / ray_displacement;
        let Vec2 {
            x: left_at,
            y: bottom_at,
        } = (self.min - ray_start) * divide_by_ray_displacement;
        let Vec2 {
            x: right_at,
            y: top_at,
        } = (self.max - ray_start) * divide_by_ray_displacement;

        // Early return: no NaNs allowed. (Can't remember what causes NaNs here
        // tho. That's 0.0/0.0. Think thru it later maybe.)
        if left_at.is_nan() || right_at.is_nan() || bottom_at.is_nan() || top_at.is_nan() {
            return None;
        }

        // Next, we sort those line intersections to account for the direction
        // of the ray.
        let (near_x_at, far_x_at, near_x_side) = if left_at < right_at {
            (left_at, right_at, Side::Left)
        } else {
            (right_at, left_at, Side::Right)
        };
        let (near_y_at, far_y_at, near_y_side) = if bottom_at < top_at {
            (bottom_at, top_at, Side::Bottom)
        } else {
            (top_at, bottom_at, Side::Top)
        };

        // Early return: line never intersects the rectangle.
        if near_x_at > far_y_at || near_y_at > far_x_at {
            return None;
        }

        // So: the line intersects the RECTANGLE BODY, not just the lines it
        // defines. Find the normalized times where it enters and exits. You're
        // only inside a rectangle while you're on the in-side of every bounding
        // line; therefore, you enter when you cross the LAST near line, and you
        // exit when you cross the FIRST far line.
        let (near_at, near_side) = if near_x_at > near_y_at {
            (near_x_at, near_x_side)
        } else {
            (near_y_at, near_y_side)
        };
        // let near_at = near_x_at.max(near_y_at);
        let far_at = far_x_at.min(far_y_at);

        // Early return: line intersects rectangle, but it's BEHIND the ray origin.
        if far_at <= 0.0 {
            return None;
        }

        // So: the RAY intersects the rectangle, not just the line defined by
        // the ray. Now we determine the contact point and normal! NOTA BENE:
        // this doesn't necessarily mean the LINE SEGMENT that defined the ray
        // strikes the rectangle. Caller is responsible for sorting that out!
        let contact_point = ray_displacement * near_at + ray_start;
        // TODO: Check whether using the actual contact side absolute location
        // makes a difference, vs. using normalized time here.
        let normal = match near_side {
            Side::Top => Vec2::Y,
            Side::Bottom => Vec2::NEG_Y,
            Side::Left => Vec2::NEG_X,
            Side::Right => Vec2::X,
        };
        // There's something in here about how to handle diagonal strikes, but
        // it doesn't make sense to me from dude's code; I might be missing
        // something. For now, leave it; Y wins a tie, according to the
        // "farthest near" test several expressions north of here.
        Some(Collision {
            contact_point,
            normal,
            normalized_time: near_at,
        })
    }

    /// Exactly like ray_collide, but only counts as a collision if it collides
    /// before the end of the proposed displacement -- far-future collisions if
    /// you continue along the ray aren't of interest.
    pub fn segment_collide(&self, ray_start: Vec2, ray_displacement: Vec2) -> Option<Collision> {
        let ray_collision = self.ray_collide(ray_start, ray_displacement);
        if let Some(c) = &ray_collision {
            if c.normalized_time >= 1.0 {
                return None;
            }
        }
        ray_collision
    }

    /// Return a new AbsBBox with each side expanded by the _opposite_ side of a
    /// relatively-defined Rect, to enable ray-intersection collision tests
    /// from the origin point that the provided Rect was defined against.
    pub fn expand_for_ray_test(&self, other: &Rect) -> Self {
        AbsBBox {
            min: self.min - other.max, // subtract bc... draw a diagram & you'll see.
            max: self.max - other.min,
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

/// Collidable solid marker component... but you also need a position Vec3 and a
/// size Vec2 from somewhere.
#[derive(Component)]
pub struct Solid;

/// Wall bundle for tilemap walls
#[derive(Bundle)]
pub struct Wall {
    solid: Solid,
    walkbox: Walkbox,
    offset: PhysOffset,
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
            offset: PhysOffset(translation_offset),
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
/// Marker component for hitbox debug mesh
#[derive(Component, Default)]
pub struct HitboxDebug;
/// Marker component for origin debug mesh
#[derive(Component, Default)]
pub struct OriginDebug;

// TODO: Maybe just convert this to a local resource for the debug spawner and
// use .entry().or() to do the initialization.
pub fn setup_debug_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut debug_assets: ResMut<DebugAssets>,
) {
    let debug_box_mesh = meshes.add(Mesh::from(shape::Quad::default()));
    let walkbox_material = materials.add(ColorMaterial::from(Color::rgba(0.5, 0.0, 0.5, 0.6)));
    let hitbox_material = materials.add(ColorMaterial::from(Color::rgba(0.8, 0.0, 0.0, 0.6)));
    let origin_material = materials.add(ColorMaterial::from(Color::rgba(1.0, 1.0, 1.0, 1.0)));
    debug_assets.insert("debug_box_mesh".to_string(), debug_box_mesh.clone_untyped());
    debug_assets.insert(
        "walkbox_material".to_string(),
        walkbox_material.clone_untyped(),
    );
    debug_assets.insert(
        "hitbox_material".to_string(),
        hitbox_material.clone_untyped(),
    );
    debug_assets.insert(
        "origin_material".to_string(),
        origin_material.clone_untyped(),
    );
}

/// Add debug mesh children to newly added collidable entities, so I can see
/// where their boundaries are. (Toggle visibility with inspector).
pub fn spawn_collider_debugs(
    new_collider_q: Query<Entity, Or<(Added<Solid>, Added<Walkbox>)>>,
    mut commands: Commands,
    debug_assets: Res<DebugAssets>,
) {
    if !new_collider_q.is_empty() {
        let (Some(mesh), Some(walkbox_material), Some(_hitbox_material), Some(origin_material)) = (
            debug_assets
                .get("debug_box_mesh")
                .map(|x| Mesh2dHandle(x.clone().typed::<Mesh>())),
            debug_assets
                .get("walkbox_material")
                .map(|x| x.clone().typed::<ColorMaterial>()),
            debug_assets
                .get("hitbox_material")
                .map(|x| x.clone().typed::<ColorMaterial>()),
            debug_assets
                .get("origin_material")
                .map(|x| x.clone().typed::<ColorMaterial>()),
        ) else {
            warn!("Hey!! I couldn't get ahold of the collider debug assets for some reason!!");
            return;
        };

        for collider in new_collider_q.iter() {
            commands.entity(collider).with_children(|parent| {
                // Spawn walkbox debugs
                parent.spawn(WalkboxDebugBundle {
                    mesh_bundle: MaterialMesh2dBundle {
                        mesh: mesh.clone(),
                        material: walkbox_material.clone(),
                        visibility: Visibility::Inherited,
                        ..default()
                    },
                    marker: WalkboxDebug,
                });

                // Spawn hitbox debugs... eventually

                // Spawn origin debugs: marker child with two mesh bundle grandkids forming a crosshair
                parent
                    .spawn((
                        OriginDebug,
                        SpatialBundle {
                            visibility: Visibility::Inherited,
                            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 40.0)),
                            ..default()
                        },
                    ))
                    .with_children(|origin| {
                        origin.spawn(MaterialMesh2dBundle {
                            mesh: mesh.clone(),
                            material: origin_material.clone(),
                            visibility: Visibility::Inherited,
                            transform: Transform::from_scale(Vec3::new(3.0, 1.0, 1.0)),
                            ..default()
                        });
                        origin.spawn(MaterialMesh2dBundle {
                            mesh: mesh.clone(),
                            material: origin_material.clone(),
                            visibility: Visibility::Inherited,
                            transform: Transform::from_scale(Vec3::new(1.0, 3.0, 1.0)),
                            ..default()
                        });
                    });
            });
        }
    }
}

/// Update size and position of collider debug meshes, since walkboxes can
/// change frame-by-frame.
pub fn debug_walkboxes_system(
    collider_q: Query<(Entity, &Walkbox)>,
    mut debug_mesh_q: Query<(&Parent, &mut Transform, &mut Visibility), With<WalkboxDebug>>,
    debug_settings: Res<DebugSettings>,
) {
    for (parent, mut transform, mut visibility) in debug_mesh_q.iter_mut() {
        let Ok((_, walkbox)) = collider_q.get(parent.get()) else {
            info!("?!?! tried to debug walkbox of some poor orphaned debug entity.");
            continue;
        };
        if debug_settings.debug_walkboxes {
            // Unconditional, not inherited:
            *visibility = Visibility::Visible;
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
            *visibility = Visibility::Hidden;
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
