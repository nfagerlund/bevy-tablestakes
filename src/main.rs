use bevy::{
    // diagnostic::{
    //     Diagnostics,
    //     FrameTimeDiagnosticsPlugin
    // },
    prelude::*,
    utils::Duration,
    sprite::Rect,
    render::texture::ImageSettings, // remove eventually, was added to prelude shortly after 0.8
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
    // InspectorPlugin,
};
use crate::input::*;
use crate::char_animation::*;
use crate::compass::*;

mod hellow;
mod junk;
mod input;
mod char_animation;
mod compass;

const PIXEL_SCALE: f32 = 1.0;

fn main() {
    App::new()
        // vv needs to go before DefaultPlugins.
        .insert_resource(WindowDescriptor {
            present_mode: bevy::window::PresentMode::Fifo,
            cursor_visible: true,
            // mode: bevy::window::WindowMode::BorderlessFullscreen,
            // width: 1920.0,
            // height: 1080.0,
            ..Default::default()
        })
        .insert_resource(ImageSettings::default_nearest()) // prevents blurry sprites
        .add_plugins(DefaultPlugins)
        .add_plugin(CharAnimationPlugin)
        .add_plugin(TestCharAnimationPlugin)
        .insert_resource(RecentFrameTimes{ buffer: VecDeque::new() })
        .insert_resource(SmoothedTime {
            delta: Duration::new(0, 0),
        })
        .insert_resource(StaticTime)
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

        // LDTK STUFF
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)

        // CAMERA
        .add_startup_system(setup_camera)

        // PLAYER STUFF
        .add_startup_system(setup_player)
        .add_system(connect_gamepads_system)
        .add_system(move_player_system)
        // .add_system(charanm_test_animate_system)
        .add_system(dumb_move_camera_system.after(move_player_system))
        .add_system(snap_pixel_positions_system.after(dumb_move_camera_system))

        // OK BYE!!!
        .run();
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

struct StaticTime;

impl StaticTime {
    fn delta_seconds(&self) -> f32 {
        1./60.
    }
    fn delta(&self) -> Duration {
        Duration::new(1, 0) / 60
    }
}

fn tile_info_barfing_system(
    keys: Res<Input<KeyCode>>,
    tile_query: Query<(&IntGridCell, &GridCoords, &Transform)>,
    level_query: Query<(&Handle<LdtkLevel>, &Transform)>,
) {
    if keys.just_pressed(KeyCode::B) {
        for (gridcell, _coords, transform) in tile_query.iter() {
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
    // time: Res<StaticTime>,
    // time: Res<SmoothedTime>,
    mut player_q: Query<(&mut SubTransform, &mut MoveRemainder, &Speed, &Walkbox), With<Player>>,
    solids_q: Query<(&Transform, &Walkbox), With<Solid>>,
    // ^^ Hmmmmmm probably gonna need a QuerySet later for this. In the meantime
    // I can probably get away with it temporarily.
) {
    // Take the chance to exit early if there's no suitable player:
    let Ok((mut player_tf, mut move_remainder, speed, walkbox)) = player_q.get_single_mut()
    else { // rust 1.65 baby
        println!("Zero players found! Probably missing walkbox or something.");
        return;
    };

    let delta = time.delta_seconds();

    // get movement intent
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

    // move, maybe! TODO: multiplayer :|
    // Cribbing from this Maddy post:
    // https://maddythorson.medium.com/celeste-and-towerfall-physics-d24bd2ae0fc5
    // TODO: spatial partitioning so the list of solids isn't so stupid huge.
    let solids: Vec<AbsBBox> = solids_q.iter().map(|(transform, walkbox)| {
        let origin = transform.translation.truncate();
        AbsBBox::from_rect(walkbox.0, origin)
    }).collect();

    move_remainder.0 += movement * speed.0 * delta;
    let move_pixels = move_remainder.0.round();
    move_remainder.0 -= move_pixels;

    let mut move_x = move_pixels.x;
    let sign_x = move_x.signum();
    while move_x != 0. {
        let next_loc = player_tf.translation + Vec3::new(sign_x, 0., 0.);
        let next_origin = next_loc.truncate();
        let next_box = AbsBBox::from_rect(walkbox.0, next_origin);
        if let None = solids.iter().find(|s| s.collide(next_box)) {
            player_tf.translation.x += sign_x;
            move_x -= sign_x;
        } else {
            // Hit a wall, theoretically we should do something additional but for now,
            break;
        }
    }
    let mut move_y = move_pixels.y;
    let sign_y = move_y.signum();
    while move_y != 0. {
        let next_loc = player_tf.translation + Vec3::new(0., sign_y, 0.);
        let next_origin = next_loc.truncate();
        let next_box = AbsBBox::from_rect(walkbox.0, next_origin);
        if let None = solids.iter().find(|s| s.collide(next_box)) {
            player_tf.translation.y += sign_y;
            move_y -= sign_y;
        } else {
            // Hit a wall, theoretically we should do something additional but for now,
            break;
        }
    }

    // Old version of move:
    // for (mut player_tf, speed) in player_q.iter_mut() {
    //     player_tf.translation += (movement * speed.0 * delta).extend(0.0);
    // }
}

fn move_camera_system(
    time: Res<Time>,
    // time: Res<StaticTime>,
    // time: Res<SmoothedTime>,
    mut params: ParamSet<(
        Query<&SubTransform, With<Player>>,
        Query<&mut SubTransform, With<Camera>>
    )>,
) {
    let delta = time.delta_seconds();
    let player_pos = params.p0().single().translation.truncate();
    // let player_pos = player_tf.translation.truncate();
    // let mut camera_tf = query.q1().get_single_mut().unwrap();
    for mut camera_tf in params.p1().iter_mut() {
        let camera_pos = camera_tf.translation.truncate();
        let camera_distance = player_pos - camera_pos;
        let follow_amount = if camera_distance.length() <= 1.0 {
            camera_distance
        } else {
            (camera_distance * 4.0 * delta).round()
        };
        camera_tf.translation += follow_amount.extend(0.0);
        // let camera_z = camera_tf.translation.z;
        // camera_tf.translation = player_pos.extend(camera_z);
        // ...and then you'd do room boundaries clamping, screenshake, etc.
    }
}

fn dumb_move_camera_system(
    mut params: ParamSet<(
        Query<&SubTransform, With<Player>>,
        Query<&mut SubTransform, With<Camera>>
    )>,
) {
    let player_pos = params.p0().single().translation;
    let mut camera_q = params.p1();
    let mut camera_tf = camera_q.single_mut();
    camera_tf.translation.x = player_pos.x;
    camera_tf.translation.y = player_pos.y;
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

fn setup_camera(
    mut commands: Commands,
) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scale = 1.0/3.0;
    commands.spawn_bundle(camera_bundle)
        .insert(SubTransform{ translation: Vec3::new(0.0, 0.0, 999.0) });
        // ^^ hack: I looked up the Z coord on new_2D and fudged it so we won't accidentally round it to 1000.
}

fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let anim_handle: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRun.aseprite");

    // IT'S THE PLAYER, GIVE IT UP!!
    commands
        .spawn_bundle(SpriteSheetBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0))
                .with_scale(Vec3::splat(PIXEL_SCALE)),
            ..Default::default()
        })

        // --- New animation system
        .insert(CharAnimationState::new(anim_handle, Dir::E))

        .insert(SubTransform{ translation: Vec3::new(0.0, 0.0, 3.0) })
        .insert(MoveRemainder(Vec2::ZERO))
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

#[derive(Component)]
struct MoveRemainder(Vec2);

/// Additional transform component for things whose movements should be synced to hard pixel boundaries.
#[derive(Component, Inspectable)]
struct SubTransform {
    translation: Vec3,
}

/// BBox defining the space an entity takes up on the ground.
#[derive(Component)]
struct Walkbox(Rect);

/// BBox defining the space where an entity can be hit by attacks.
#[derive(Component)]
struct Hitbox(Rect);
// ...and then eventually I'll want Swingbox for attacks, but, tbh I have no
// idea how to best handle that yet. Is that even a component? Or is it a larger
// data structure associated with an animation or something?

fn centered_rect(width: f32, height: f32) -> Rect {
    let min = Vec2::new(-width/2., -height/2.);
    let max = Vec2::new(width/2., height/2.);
    Rect {
        min,
        max,
    }
}

fn bottom_centered_rect(width: f32, height: f32) -> Rect {
    let min = Vec2::new(-width/2., 0.);
    let max = Vec2::new(width/2., height);
    Rect {
        min,
        max,
    }
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
    fn from_rect(rect: Rect, origin: Vec2) -> Self {
        Self {
            min: rect.min + origin,
            max: rect.max + origin,
        }
    }

    /// Check whether an absolutely positioned bbox overlaps with another one.
    fn collide(&self, other: Self) -> bool {
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


/// Collidable solid component... but you also need a position Vec3 and a size Vec2 from somewhere.
#[derive(Component)]
struct Solid;

/// Wall bundle for tilemap walls
#[derive(Bundle)]
struct Wall {
    solid: Solid,
    walkbox: Walkbox,
    // transform: Transform, // This is needed, but it's handled by the plugin.
}

// Custom impl instead of derive bc... you'll see!
impl LdtkIntCell for Wall {
    fn bundle_int_cell(_: IntGridCell, layer_instance: &LayerInstance) -> Self {
        // there!! v. proud of finding this, the example just cheated w/ prior knowledge.
        let grid_size = layer_instance.grid_size as f32;
        Wall {
            solid: Solid,
            // the plugin puts tile anchor points in the center:
            walkbox: Walkbox(centered_rect(grid_size, grid_size)),
        }
    }
}
