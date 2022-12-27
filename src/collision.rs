use std::collections::HashMap;

use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};
use bevy_ecs_ldtk::prelude::*;
use bevy_inspector_egui::Inspectable;

/// BBox defining the space an entity takes up on the ground.
#[derive(Component, Inspectable)]
pub struct Walkbox(pub Rect);

/// BBox defining the space where an entity can be hit by attacks.
#[derive(Component, Inspectable)]
pub struct Hitbox(pub Rect);
// ...and then eventually I'll want Hurtbox for attacks, but, tbh I have no
// idea how to best handle that yet. Is that even a component? Or is it a larger
// data structure associated with an animation or something?

pub fn centered_rect(width: f32, height: f32) -> Rect {
    let min = Vec2::new(-width / 2., -height / 2.);
    let max = Vec2::new(width / 2., height / 2.);
    Rect { min, max }
}

pub fn bottom_centered_rect(width: f32, height: f32) -> Rect {
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
        if self.min.x > other.max.x {
            // we're right of other
            false
        } else if self.max.x < other.min.x {
            // we're left of other
            false
        } else if self.max.y < other.min.y {
            // we're below other
            false
        } else if self.min.y > other.max.y {
            // we're above other
            false
        } else {
            // guess we're colliding! ¯\_(ツ)_/¯
            true
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

// Can't spawn child entities in bundle_int_cell, alas, so need an after-the-fact setup
pub fn spawn_wall_tile_collider_debugs(
    new_wall_q: Query<Entity, Added<Solid>>,
    mut commands: Commands,
    debug_assets: Res<DebugAssets>,
) {
    let (Some(mesh_untyped), Some(material_untyped)) = (debug_assets.get("walkbox_mesh"), debug_assets.get("walkbox_material"))
    else {
        warn!("Hey!! I couldn't get ahold of the collider debug assets for some reason!!");
        return;
    };
    let material = material_untyped.clone().typed::<ColorMaterial>();
    let mesh = Mesh2dHandle(mesh_untyped.clone().typed::<Mesh>());

    for wall in new_wall_q.iter() {
        commands.entity(wall).with_children(|parent| {
            parent.spawn(WalkboxDebugBundle {
                mesh_bundle: MaterialMesh2dBundle {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    visibility: Visibility { is_visible: false },
                    ..default()
                },
                marker: WalkboxDebug,
            });
        });
    }
}

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
            // offset our translation by the difference between the walkbox's
            // center and it's actual anchor point.
            let size = walkbox.0.max - walkbox.0.min;
            let center = walkbox.0.min + size / 2.0;
            // and of course, the anchor point of the rect is (0,0), by definition.
            transform.scale = size.extend(1.0);
            transform.translation = center.extend(1.0);
        } else {
            visibility.is_visible = false;
            // we're done
        }
    }
}
