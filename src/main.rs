use bevy::{
    diagnostic::{
        // Diagnostics,
        FrameTimeDiagnosticsPlugin},
    prelude::*,
    utils::Duration,
};
use bevy_ecs_ldtk::prelude::*;
// use bevy_ecs_tilemap::prelude::*;
use std::collections::VecDeque;

mod hellow;
mod junk;

const PIXEL_SCALE: f32 = 3.0;

fn main() {
    App::new()
        // vv needs to go before DefaultPlugins.
        .insert_resource(WindowDescriptor {
            vsync: true,
            cursor_visible: false,
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
        .add_system_to_stage(CoreStage::PreUpdate, time_smoothing_system)
        .add_plugin(LdtkPlugin)
        // .add_plugin(hellow::HelloPlugin)
        // .add_plugin(FrameTimeDiagnosticsPlugin)
        // .add_startup_system(junk::setup_fps_debug)
        // .add_system(junk::update_fps_debug_system)
        // .add_system(junk::debug_z_system)
        .add_startup_system(setup_sprites)
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .add_system(animate_sprites_system)
        .add_system(connect_gamepads_system)
        .add_system(move_player_system.label(Movements))
        .add_system(move_camera_system.label(CamMovements).after(Movements))
        .add_system(snap_pixel_positions_system.after(CamMovements))
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


// Input time!

// helper function: forward the axes resource (and a gamepad id) to it, get a vec back.
// Note: `gilrs`, Bevy's gamepad library, only supports Xinput on windows. boo.
fn get_gamepad_movement_vector(gamepad: Gamepad, axes: Res<Axis<GamepadAxis>>) -> Option<Vec2> {
    let x_axis = GamepadAxis(gamepad, GamepadAxisType::LeftStickX);
    let y_axis = GamepadAxis(gamepad, GamepadAxisType::LeftStickY);
    let x = axes.get(x_axis)?;
    let y = axes.get(y_axis)?;
    Some(Vec2::new(x, y))
}

// helper function: forward keycodes to it, get a vec back.
fn get_kb_movement_vector(keys: Res<Input<KeyCode>>) -> Vec2 {
    let mut x = 0f32;
    let mut y = 0f32;
    if keys.pressed(KeyCode::Left) {
        x -= 1.0;
    }
    if keys.pressed(KeyCode::Right) {
        x += 1.0;
    }
    if keys.pressed(KeyCode::Up) {
        y += 1.0; // bc, opposite of other engines so far
    }
    if keys.pressed(KeyCode::Down) {
        y -= 1.0;
    }
    Vec2::new(x, y)
}

fn move_player_system(
    active_gamepad: Option<Res<ActiveGamepad>>,
    axes: Res<Axis<GamepadAxis>>,
    keys: Res<Input<KeyCode>>,
    // time: Res<Time>,
    time: Res<SmoothedTime>,
    mut query: Query<(&mut SubpixelTranslation, &Speed), With<Player>>,
) {
    let delta = time.delta_seconds();
    let mut gamepad_movement = None;
    if let Some(ActiveGamepad(pad_id)) = active_gamepad.as_deref() {
        gamepad_movement = get_gamepad_movement_vector(*pad_id, axes);
    }
    let movement = match gamepad_movement {
        Some(mvmt) => mvmt,
        None => get_kb_movement_vector(keys),
    };
    for (mut player_pos, speed) in query.iter_mut() {
        player_pos.0 += (movement * speed.0 * delta).extend(0.0);
    }
}

fn move_camera_system(
    // time: Res<Time>,
    time: Res<SmoothedTime>,
    mut query: QuerySet<(
        QueryState<&SubpixelTranslation, With<Player>>,
        QueryState<&mut SubpixelTranslation, With<MainCamera>>
    )>,
) {
    let delta = time.delta_seconds();
    let player_tf = query.q0().get_single().unwrap();
    let player_pos = player_tf.0.truncate();
    // let mut camera_tf = query.q1().get_single_mut().unwrap();
    for mut camera_tf in query.q1().iter_mut() {
        let camera_pos = camera_tf.0.truncate();
        let follow_amount = (player_pos - camera_pos) * 4.0 * delta;
        camera_tf.0 += follow_amount.extend(0.0);
        // let camera_z = camera_tf.0.z;
        // camera_tf.0 = player_pos.extend(camera_z);
        // ...and then you'd do room boundaries clamping, screenshake, etc.
    }
}

fn snap_pixel_positions_system(
    mut query: Query<(&SubpixelTranslation, &mut Transform)>,
) {
    let global_scale = Vec3::new(PIXEL_SCALE, PIXEL_SCALE, 1.0);
    for (subpixel_tl, mut pixel_tf) in query.iter_mut() {
        pixel_tf.translation = (global_scale * subpixel_tl.0).floor();
    }
}

fn connect_gamepads_system(
    mut commands: Commands,
    active_gamepad: Option<Res<ActiveGamepad>>,
    mut gamepad_events: EventReader<GamepadEvent>,
    // ^^ eventreader params have to be mutable because reading events immutably
    // still updates an internal tracking cursor on the reader instance. cool.
) {
    for GamepadEvent(id, kind) in gamepad_events.iter() {
        match kind {
            GamepadEventType::Connected => {
                info!("pad up: {:?}", id);
                // let's see, I de-focused the cookbook tab, so what do *I* want to have happen?
                // First pad in gets it, but if another pad hits start, it'll take over. Nice.
                if active_gamepad.is_none() {
                    commands.insert_resource(ActiveGamepad(*id));
                }
            },
            GamepadEventType::Disconnected => {
                info!("pad down: {:?}", id);
                // byeeee
                // ok, I'm back to the example code, what's going on here:
                if let Some(ActiveGamepad(old_id)) = active_gamepad.as_deref() {
                    if old_id == id {
                        commands.remove_resource::<ActiveGamepad>();
                        // haven't really had to turbofish before now. zoom zoom glub glub.
                    }
                }
            },
            GamepadEventType::ButtonChanged(GamepadButtonType::Start, val) if *val == 1.0 => {
                info!("Pressed start: {:?}", id);
                // If there's an active gamepad...
                if let Some(ActiveGamepad(old_id)) = active_gamepad.as_deref() {
                    // ...but it's not the one you just pressed start on...
                    if old_id != id {
                        // ...then let it take over.
                        commands.insert_resource(ActiveGamepad(*id));
                        // per the cheatbook: "If you insert a resource of a
                        // type that already exists, it will be overwritten."
                    }
                }
            },
            // ignore other events for now, ^H^H^H (never mind) but fyi see
            // examples/input/gamepad_input_events.rs in bevy. The
            // ButtonChanged(button_type, value) and AxisChanged(axis_type,
            // value) all return new values as floats, I guess so you can handle
            // buttons and analog axes with similar code.
            _ => {},
        }
    }
}

// animation time!

fn animate_sprites_system(
    // time: Res<Time>,
    time: Res<SmoothedTime>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    mut query: Query<(&mut Timer, &mut TextureAtlasSprite, &Handle<TextureAtlas>)>,
    // ^^ ok, the timer I added myself, and the latter two were part of the bundle.
) {
    for (mut timer, mut sprite, texture_atlas_handle) in query.iter_mut() {
        timer.tick(time.delta()); // ok, I remember you. advance the timer.
        if timer.finished() {
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
        transform: Transform::from_scale(Vec3::splat(3.0)),
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
    // camera_bundle.orthographic_projection.scale = 1.0/3.0;
    commands.spawn_bundle(camera_bundle)
        .insert(SubpixelTranslation(Vec3::new(0.0, 0.0, 999.0)))
        // ^^ hack: I looked up the Z coord on new_2D and fudged it so we won't accidentally round it to 1000.
        .insert(MainCamera);
    commands.spawn_bundle(UiCameraBundle::default());
    commands
        .spawn_bundle(SpriteSheetBundle {
            texture_atlas: texture_atlas_handle,
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0))
                .with_scale(Vec3::splat(PIXEL_SCALE)),
            ..Default::default()
        })
        .insert(SubpixelTranslation(Vec3::new(0.0, 0.0, 3.0)))
        .insert(Timer::from_seconds(0.1, true))
        // ^^ 0.1 = inverse FPS. Could be way more ergonomic.
        .insert(Speed(120.0))
        .insert(Player);
}

// Structs and crap!

/// Resource for storing the active gamepad
struct ActiveGamepad(Gamepad);

/// Label for stages that move things around the level.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, SystemLabel)]
struct Movements;

/// Label for stages that move the camera around the level.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, SystemLabel)]
struct CamMovements;

/// Marker component for a spawned LdtkWorldBundle
#[derive(Component)]
pub struct LdtkWorld;

/// Marker component for the player
#[derive(Component)]
pub struct Player;

/// Marker component for main camera
#[derive(Component)]
struct MainCamera;

/// Speed in pixels... per... second?
#[derive(Component)]
struct Speed(f32);

/// Additional transform component for things whose movements should be synced to hard pixel boundaries.
#[derive(Component)]
struct SubpixelTranslation(Vec3);
