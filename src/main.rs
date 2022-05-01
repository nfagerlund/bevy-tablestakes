use bevy::{
    // diagnostic::{
    //     Diagnostics,
    //     FrameTimeDiagnosticsPlugin
    // },
    prelude::*,
    utils::Duration,
    render::camera::Camera2d,
};
use bevy_ecs_ldtk::prelude::*;
// use bevy_ecs_tilemap::prelude::*;
use std::{
    collections::VecDeque,
    // f32::consts::PI
};
use bevy_inspector_egui::{
    WorldInspectorPlugin,
    Inspectable,
    RegisterInspectable,
    InspectorPlugin,
};
use crate::input::*;

mod hellow;
mod junk;
mod input;

const PIXEL_SCALE: f32 = 1.0;

fn main() {
    App::new()
        // vv needs to go before DefaultPlugins.
        .insert_resource(WindowDescriptor {
            present_mode: bevy::window::PresentMode::Mailbox,
            cursor_visible: true,
            // mode: bevy::window::WindowMode::BorderlessFullscreen,
            // width: 1920.0,
            // height: 1080.0,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .insert_resource(RecentFrameTimes{ buffer: VecDeque::new() })
        .insert_resource(SmoothedTime {
            delta: Duration::new(0, 0),
        })
        // .add_system_to_stage(CoreStage::PreUpdate, time_smoothing_system)
        .add_plugin(LdtkPlugin)

        // NOISY DEBUG STUFF
        // .add_plugin(FrameTimeDiagnosticsPlugin)
        // .add_startup_system(junk::setup_fps_debug)
        // .add_system(junk::update_fps_debug_system)
        // .add_system(junk::debug_z_system)
        .add_system(tile_info_barfing_system)

        // INSPECTOR STUFF
        .add_plugin(WorldInspectorPlugin::new())
        .register_inspectable::<SubTransform>()
        .insert_resource(WonderWall::default())
        .add_plugin(InspectorPlugin::<WonderWall>::new())
        .add_system(look_for_walls_system)

        // LDTK STUFF
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)

        // PLAYER STUFF
        .add_startup_system(setup_sprites)
        .add_system(animate_sprites_system)
        .add_system(connect_gamepads_system)
        .add_system(move_player_system)
        .add_system(move_camera_system.after(move_player_system))
        .add_system(snap_pixel_positions_system.after(move_camera_system))

        // OK BYE!!!
        .run();
}

/// resource for collision yelling
#[derive(Inspectable, Default)]
struct WonderWall {
    tile_entity: Option<Entity>,
    tile_grid_coords: Option<IVec2>,
    num_walls: usize,
}

struct RecentFrameTimes {
    buffer: VecDeque<Duration>,
}
struct SmoothedTime {
    delta: Duration,
}

impl SmoothedTime {
    fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }
    fn delta(&self) -> Duration {
        self.delta
    }
}

fn look_for_walls_system(
    mut wonderwall: ResMut<WonderWall>,
    walls_q: Query<(&BBox, &Transform, &GridCoords, Entity), With<Solid>>,
    player_q: Query<&Transform, With<Player>>,
) {
    let player_transform = player_q.single();
    wonderwall.tile_entity = None;
    wonderwall.tile_grid_coords = None;
    wonderwall.num_walls = walls_q.iter().count();
    for (bbox, transform, grid_coords, entity) in walls_q.iter() {
        if point_in_bbox(player_transform.translation.truncate(), transform.translation.truncate(), bbox.size) {
            wonderwall.tile_entity = Some(entity);
            wonderwall.tile_grid_coords = Some(IVec2::new(grid_coords.x, grid_coords.y));
        }
    }
}

// This is naive and assumes the pivot point in in the center of the bbox.
fn point_in_bbox(point: Vec2, bbox_location: Vec2, bbox_size: Vec2) -> bool {
    point.x >= bbox_location.x - bbox_size.x / 2.
        && point.x <= bbox_location.x + bbox_size.x / 2.
        && point.y >= bbox_location.y - bbox_size.y / 2.
        && point.y <= bbox_location.y + bbox_size.y / 2.
}

#[test]
fn point_in_bbox_test() {
    let loc = Vec2::new(5.0, 5.0);
    let bb_size = BBox{ size: Vec2::new(16.0, 16.0) };
    let bb_in = Vec2::new(6.0, 6.0);
    let bb_out = Vec2::new(90.0, 90.0);
    assert!(point_in_bbox(loc, bb_in, bb_size.size));
    assert!(!point_in_bbox(loc, bb_out, bb_size.size));
}

fn tile_info_barfing_system(
    keys: Res<Input<KeyCode>>,
    tile_query: Query<(&IntGridCell, &GridCoords, &Transform)>,
    level_query: Query<(&Handle<LdtkLevel>, &Transform)>,
) {
    if keys.just_pressed(KeyCode::B) {
        for (gridcell, coords, transform) in tile_query.iter() {
            info!("{:?} at {:?}", gridcell, transform);
        }
        for (level, transform) in level_query.iter() {
            info!("level {:?} at {:?}", level, transform);
        }
    }
}

// Smooth out delta time before doing anything with it. This is unoptimized, but that might not matter.
fn time_smoothing_system(
    time: Res<Time>,
    mut recent_time: ResMut<RecentFrameTimes>,
    mut smoothed_time: ResMut<SmoothedTime>,
) {
    let window: usize = 11;
    let delta = time.delta();
    recent_time.buffer.push_back(delta);
    if recent_time.buffer.len() >= window + 1 {
        recent_time.buffer.pop_front();
        let mut sorted: Vec<Duration> = recent_time.buffer.clone().into();
        sorted.sort_unstable();
        let sum = &sorted[2..(window - 2)].iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
        smoothed_time.delta = *sum / (window as u32 - 4);
    } else {
        smoothed_time.delta = delta;
    }
}

fn move_player_system(
    active_gamepad: Option<Res<ActiveGamepad>>,
    axes: Res<Axis<GamepadAxis>>,
    keys: Res<Input<KeyCode>>,
    time: Res<Time>,
    // time: Res<SmoothedTime>,
    mut query: Query<(&mut SubTransform, &Speed), With<Player>>,
) {
    let delta = time.delta_seconds();
    let mut gamepad_movement = None;
    if let Some(ActiveGamepad(pad_id)) = active_gamepad.as_deref() {
        gamepad_movement = get_gamepad_movement_vector(*pad_id, axes);
    }
    let movement = match gamepad_movement {
        Some(mvmt) => {
            if mvmt.length() > 0.0 {
                mvmt
            } else {
                get_kb_movement_vector(keys)
            }
        },
        None => get_kb_movement_vector(keys),
    };
    for (mut player_tf, speed) in query.iter_mut() {
        player_tf.translation += (movement * speed.0 * delta).extend(0.0);
    }
}

fn move_camera_system(
    time: Res<Time>,
    // time: Res<SmoothedTime>,
    mut params: ParamSet<(
        Query<&SubTransform, With<Player>>,
        Query<&mut SubTransform, With<Camera2d>>
    )>,
) {
    let delta = time.delta_seconds();
    let player_pos = params.p0().single().translation.truncate();
    // let player_pos = player_tf.translation.truncate();
    // let mut camera_tf = query.q1().get_single_mut().unwrap();
    for mut camera_tf in params.p1().iter_mut() {
        let camera_pos = camera_tf.translation.truncate();
        let follow_amount = (player_pos - camera_pos) * 4.0 * delta;
        camera_tf.translation += follow_amount.extend(0.0);
        // let camera_z = camera_tf.translation.z;
        // camera_tf.translation = player_pos.extend(camera_z);
        // ...and then you'd do room boundaries clamping, screenshake, etc.
    }
}

fn snap_pixel_positions_system(
    mut query: Query<(&SubTransform, &mut Transform)>,
) {
    // let global_scale = Vec3::new(PIXEL_SCALE, PIXEL_SCALE, 1.0);
    for (sub_tf, mut pixel_tf) in query.iter_mut() {
        // pixel_tf.translation = (global_scale * sub_tf.translation).floor();
        pixel_tf.translation = sub_tf.translation;
    }
}

// animation time!

fn animate_sprites_system(
    time: Res<Time>,
    // time: Res<SmoothedTime>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    mut query: Query<(&mut SpriteTimer, &mut TextureAtlasSprite, &Handle<TextureAtlas>)>,
    // ^^ ok, the timer I added myself, and the latter two were part of the bundle.
) {
    for (mut sprite_timer, mut sprite, texture_atlas_handle) in query.iter_mut() {
        sprite_timer.timer.tick(time.delta()); // ok, I remember you. advance the timer.
        if sprite_timer.timer.finished() {
            let texture_atlas = texture_atlases.get(texture_atlas_handle).unwrap(); // uh ok. btw, how do we avoid the unwraps in this runtime?
            sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
            // ^^ Ah. OK. We're doing some realll basic flipbooking here. But also, note that the TextureAtlasSprite struct ONLY has color/index/flip_(x|y)/custom_size props, it's meant to always be paired with a textureatlas handle and it doesn't hold its own reference to one. ECS lifestyles.
        }
    }
}

fn setup_level(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn_bundle(LdtkWorldBundle {
        ldtk_handle: asset_server.load("kittytown.ldtk"),
        transform: Transform::from_scale(Vec3::splat(PIXEL_SCALE)),
        ..Default::default()
    }).insert(LdtkWorld);
}

fn setup_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    let texture_handle: Handle<Image> = asset_server.load("sprites/sPlayerRun_strip32.png");
    // vv AH ha, and here's the bit I would want some automation for. Should be easy lol.
    let texture_atlas = TextureAtlas::from_grid(texture_handle, Vec2::new(17.0, 24.0), 32, 1);
    let texture_atlas_handle = texture_atlases.add(texture_atlas);

    let mut camera_bundle = OrthographicCameraBundle::new_2d();
    camera_bundle.orthographic_projection.scale = 1.0/3.0;
    commands.spawn_bundle(camera_bundle)
        .insert(SubTransform{ translation: Vec3::new(0.0, 0.0, 999.0) });
        // ^^ hack: I looked up the Z coord on new_2D and fudged it so we won't accidentally round it to 1000.
    commands.spawn_bundle(UiCameraBundle::default());
    commands
        .spawn_bundle(SpriteSheetBundle {
            texture_atlas: texture_atlas_handle,
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0))
                .with_scale(Vec3::splat(PIXEL_SCALE)),
            ..Default::default()
        })
        .insert(SubTransform{ translation: Vec3::new(0.0, 0.0, 3.0) })
        .insert(SpriteTimer{ timer: Timer::from_seconds(0.1, true) })
        // ^^ 0.1 = inverse FPS. Could be way more ergonomic.
        .insert(Speed(120.0))
        .insert(Player);
}

// Structs and crap!

/// Resource for storing the active gamepad
pub struct ActiveGamepad(Gamepad);

/// Marker component for a spawned LdtkWorldBundle
#[derive(Component)]
pub struct LdtkWorld;

/// Marker component for the player
#[derive(Component)]
pub struct Player;

/// Speed in pixels... per... second?
#[derive(Component)]
struct Speed(f32);

/// Sprite animation frame timer
#[derive(Component)]
struct SpriteTimer {
    timer: Timer,
}

/// Additional transform component for things whose movements should be synced to hard pixel boundaries.
#[derive(Component, Inspectable)]
struct SubTransform {
    translation: Vec3,
}

/// Collidable solid component... but you also need a position Vec3 and a size Vec2 from somewhere.
#[derive(Component)]
struct Solid;

/// Bounding box size component. I'm using the f32-based Vec2 because that's what
/// bevy::sprite::collide_aabb::collide() uses.
#[derive(Component)]
struct BBox {
    size: Vec2,
}

/// Wall bundle for tilemap walls
#[derive(Bundle)]
struct Wall {
    solid: Solid,
    bbox: BBox,
    // transform: Transform, // This is needed, but it's handled by the plugin.
}

// Custom impl instead of derive bc... you'll see!
impl LdtkIntCell for Wall {
    fn bundle_int_cell(_: IntGridCell, layer_instance: &LayerInstance) -> Self {
        Wall {
            solid: Solid,
            bbox: BBox {
                // there!! v. proud of finding this, the example just cheated w/ prior knowledge.
                size: Vec2::splat(layer_instance.grid_size as f32),
            },
        }
    }
}
