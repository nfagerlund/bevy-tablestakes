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
    int_grid_cell: IntGridCell,
    // transform: Transform, // This is needed, but it's handled by the plugin.
}

// Custom impl instead of derive bc... you'll see!
impl LdtkIntCell for Wall {
    fn bundle_int_cell(int_grid_cell: IntGridCell, layer_instance: &LayerInstance) -> Self {
        // there!! v. proud of finding this, the example just cheated w/ prior knowledge.
        let grid_size = layer_instance.grid_size as f32;
        Wall {
            solid: Solid,
            // the plugin puts tile anchor points in the center:
            walkbox: Walkbox(centered_rect(grid_size, grid_size)),
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
