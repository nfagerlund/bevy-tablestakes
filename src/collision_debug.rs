use crate::{
    collision::{Hitbox, Hurtbox, Solid, Walkbox},
    DebugSettings,
};
use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};

// -- COLLIDER DEBUG MESH STUFF --

#[derive(Resource)]
pub struct DebugAssets {
    box_mesh: Handle<Mesh>,
    walkbox_color: Handle<ColorMaterial>,
    hitbox_color: Handle<ColorMaterial>,
    hurtbox_color: Handle<ColorMaterial>,
    origin_color: Handle<ColorMaterial>,
}

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
/// Marker component for hurtbox debug mesh
#[derive(Component, Default)]
pub struct HurtboxDebug;
/// Marker component for origin debug mesh
#[derive(Component, Default)]
pub struct OriginDebug;

// TODO 0.13 replace these meshes with gizmos!
pub fn setup_debug_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let box_mesh = meshes.add(Mesh::from(Rectangle::default()));
    let walkbox_color = materials.add(ColorMaterial::from(Color::srgba(0.5, 0.0, 0.5, 0.6)));
    let hitbox_color = materials.add(ColorMaterial::from(Color::srgba(0.8, 0.0, 0.0, 0.6)));
    let hurtbox_color = materials.add(ColorMaterial::from(Color::srgba(0.0, 0.8, 0.0, 0.6)));
    let origin_color = materials.add(ColorMaterial::from(Color::srgba(1.0, 1.0, 1.0, 1.0)));

    commands.insert_resource(DebugAssets {
        box_mesh,
        walkbox_color,
        hitbox_color,
        hurtbox_color,
        origin_color,
    });
}

/// Add debug mesh children to newly added collidable entities, so I can see
/// where their boundaries are. (Toggle visibility with inspector).
pub fn spawn_collider_debugs(
    new_collider_q: Query<
        (
            Entity,
            Option<&Children>,
            Option<Ref<Solid>>,
            Option<Ref<Walkbox>>,
            Option<Ref<Hitbox>>,
            Option<Ref<Hurtbox>>,
        ),
        Or<(Added<Solid>, Added<Walkbox>, Added<Hitbox>, Added<Hurtbox>)>,
    >,
    old_origins_q: Query<&OriginDebug>,
    mut commands: Commands,
    assets: Res<DebugAssets>,
) {
    if !new_collider_q.is_empty() {
        for (collider, maybe_children, r_solid, r_walkbox, r_hitbox, r_hurtbox) in
            new_collider_q.iter()
        {
            let solid_added = r_solid.map_or(false, |x| x.is_added());
            let walkbox_added = r_walkbox.map_or(false, |x| x.is_added());
            let hitbox_added = r_hitbox.map_or(false, |x| x.is_added());
            let hurtbox_added = r_hurtbox.map_or(false, |x| x.is_added());

            commands.entity(collider).with_children(|parent| {
                // Maybe spawn walkbox debugs
                if solid_added || walkbox_added {
                    parent.spawn(WalkboxDebugBundle {
                        mesh_bundle: MaterialMesh2dBundle {
                            mesh: Mesh2dHandle(assets.box_mesh.clone()),
                            material: assets.walkbox_color.clone(),
                            visibility: Visibility::Inherited,
                            ..default()
                        },
                        marker: WalkboxDebug,
                    });
                }

                // Maybe spawn hitbox debugs
                if hitbox_added {
                    parent.spawn((
                        HitboxDebug,
                        MaterialMesh2dBundle {
                            mesh: Mesh2dHandle(assets.box_mesh.clone()),
                            material: assets.hitbox_color.clone(),
                            visibility: Visibility::Inherited,
                            ..default()
                        },
                    ));
                }

                // Maybe spawn hurtbox debugs
                if hurtbox_added {
                    parent.spawn((
                        HurtboxDebug,
                        MaterialMesh2dBundle {
                            mesh: Mesh2dHandle(assets.box_mesh.clone()),
                            material: assets.hurtbox_color.clone(),
                            visibility: Visibility::Inherited,
                            ..default()
                        },
                    ));
                }

                // Spawn origin debugs: marker child with two mesh bundle grandkids forming a crosshair
                // Only want to do this once, even if this is the parent's second time through this system
                // (e.g. Hitbox got added later, after walkbox)
                let spawn_origin_debug = match maybe_children {
                    Some(children) => {
                        // No existing child has the OriginDebug component:
                        !children.iter().any(|&ent| old_origins_q.get(ent).is_ok())
                    },
                    None => true,
                };
                if spawn_origin_debug {
                    parent
                        .spawn((
                            OriginDebug,
                            SpatialBundle {
                                visibility: Visibility::Inherited,
                                // z-stack: 1 below walkbox mesh
                                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 39.0)),
                                ..default()
                            },
                        ))
                        .with_children(|origin| {
                            origin.spawn(MaterialMesh2dBundle {
                                mesh: Mesh2dHandle(assets.box_mesh.clone()),
                                material: assets.origin_color.clone(),
                                visibility: Visibility::Inherited,
                                transform: Transform::from_scale(Vec3::new(3.0, 1.0, 1.0)),
                                ..default()
                            });
                            origin.spawn(MaterialMesh2dBundle {
                                mesh: Mesh2dHandle(assets.box_mesh.clone()),
                                material: assets.origin_color.clone(),
                                visibility: Visibility::Inherited,
                                transform: Transform::from_scale(Vec3::new(1.0, 3.0, 1.0)),
                                ..default()
                            });
                        });
                }
            });
        }
    }
}

/// This one gets to be much dumber than the others bc the size never
/// changes and the transform propagation is free.
pub fn debug_origins_system(
    mut debug_mesh_q: Query<&mut Visibility, With<OriginDebug>>,
    debug_settings: Res<DebugSettings>,
) {
    if debug_settings.debug_origins {
        debug_mesh_q.iter_mut().for_each(|mut v| {
            *v = Visibility::Visible;
        });
    } else {
        debug_mesh_q.iter_mut().for_each(|mut v| {
            *v = Visibility::Hidden;
        });
    }
}

// A private helper to deduplicate logic for walkbox/hitbox/hurtbox debugs
fn flip_collider_debug_meshes<'a>(
    enabled: bool,
    z_stack: f32,
    debug_meshes: impl Iterator<Item = (&'a Parent, Mut<'a, Transform>, Mut<'a, Visibility>)>,
    rect_getter: impl Fn(Entity) -> Option<Rect>,
) {
    for (parent, mut transform, mut visibility) in debug_meshes {
        if let (true, Some(active_rect)) = (enabled, rect_getter(parent.get())) {
            *visibility = Visibility::Visible;
            let size = active_rect.max - active_rect.min;
            let center = active_rect.min + size / 2.0;
            transform.scale = size.extend(1.0);
            transform.translation = center.extend(z_stack);
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Update size and position of collider debug meshes, since walkboxes etc. can
/// change frame-by-frame.
pub fn debug_collider_boxes_system(
    walkbox_q: Query<&Walkbox>,
    hitbox_q: Query<&Hitbox>,
    hurtbox_q: Query<&Hurtbox>,
    mut debug_mesh_set: ParamSet<(
        Query<(&Parent, &mut Transform, &mut Visibility), With<WalkboxDebug>>,
        Query<(&Parent, &mut Transform, &mut Visibility), With<HitboxDebug>>,
        Query<(&Parent, &mut Transform, &mut Visibility), With<HurtboxDebug>>,
    )>,
    debug_settings: Res<DebugSettings>,
) {
    // The walkbox getter uses .map, bc it has an infallible Rect inside.
    // Other getters use .and_then, bc they have Option<Rect>s inside.

    // Walkboxes
    flip_collider_debug_meshes(
        debug_settings.debug_walkboxes,
        40.0, // WAY above parent, to avoid TopDownMatter interactions
        debug_mesh_set.p0().iter_mut(),
        |e| walkbox_q.get(e).ok().map(|wb| wb.0),
    );
    // Hitboxes
    flip_collider_debug_meshes(
        debug_settings.debug_hitboxes,
        41.0,
        debug_mesh_set.p1().iter_mut(),
        |e| hitbox_q.get(e).ok().and_then(|hb| hb.0),
    );
    // Hurtboxes
    flip_collider_debug_meshes(
        debug_settings.debug_hurtboxes,
        42.0,
        debug_mesh_set.p2().iter_mut(),
        |e| hurtbox_q.get(e).ok().and_then(|hb| hb.0),
    );
}
