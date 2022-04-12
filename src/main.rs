use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_ecs_ldtk::prelude::*;
use bevy_ecs_tilemap::prelude::*;

mod hellow;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(LdtkPlugin)
        // .add_plugin(hellow::HelloPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_startup_system(setup_sprites)
        .add_startup_system(setup_fps_debug)
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .add_system(update_fps_debug_system)
        .add_system(animate_sprites_system)
        .add_system(connect_gamepads_system)
        .add_system(move_player_system.label(Movements))
        .add_system(move_camera_system.after(Movements))
        // .add_system(_debug_z_system)
        .run();
}

// Input time!

// All right, so an unfortunate thing I learned is that `gilrs`, the gamepad
// library bevy relies on, only supports Xinput on windows. That means no PS4
// controller unless you either run ds4windows or publish on steam to get
// steaminput. On the plus side, it looks like the issue is just that it's a
// volunteer-maintained project of relatively young age without maxed-out
// platform api knowledge, so maybe someone'll add it at some point. (Or maybe
// I'll have to.)

struct ActiveGamepad(Gamepad);

// helper function: forward the axes resource (and a gamepad id) to it, get a vec back.
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
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Speed), With<Player>>,
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
    for (mut player_transform, speed) in query.iter_mut() {
        player_transform.translation += (movement * speed.0 * delta).extend(0.0);
    }
}

fn move_camera_system(
    time: Res<Time>,
    mut query: QuerySet<(
        QueryState<&Transform, With<Player>>,
        QueryState<&mut Transform, With<MainCamera>>
    )>,
) {
    let delta = time.delta_seconds();
    let player_tf = query.q0().get_single().unwrap();
    let player_pos = player_tf.translation.truncate();
    // let mut camera_tf = query.q1().get_single_mut().unwrap();
    for mut camera_tf in query.q1().iter_mut() {
        let camera_pos = camera_tf.translation.truncate();
        let follow_amount = (player_pos - camera_pos) * 4.0 * delta;
        camera_tf.translation += follow_amount.extend(0.0);
        // ...and then you'd do room boundaries clamping, screenshake, etc.
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, SystemLabel)]
struct Movements;

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
    time: Res<Time>,
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

fn _debug_z_system(
    // mut local_timer: Local<Timer>,
    player_query: Query<&Transform, With<Player>>,
    world_query: Query<&Transform, With<World>>,
    level_query: Query<(Entity, &Transform, &Map)>,
) {
    let player_transform = player_query.get_single().unwrap();
    let world_transform = world_query.get_single().unwrap();
    info!("Player at: {}\n World at: {}\n", player_transform.translation, world_transform.translation);
    for (e_id, transform, map) in level_query.iter() {
        info!("  Level {:?} (map id {}) at {}\n", e_id, map.id, transform.translation);
    }
}

fn setup_level(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn_bundle(LdtkWorldBundle {
        ldtk_handle: asset_server.load("kittytown.ldtk"),
        ..Default::default()
    }).insert(World);
}

#[derive(Component)]
struct World;

fn setup_fps_debug(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let style = TextStyle {
        font: asset_server.load("fonts/m5x7.ttf"),
        font_size: 32.0,
        color: Color::rgb(0.0, 1.0, 0.0),
    };
    // borrowing this from the bevymark example
    commands.spawn_bundle(TextBundle {
        text: Text {
            sections: vec![
                TextSection {
                    value: "FPS: ".to_string(),
                    style: style.clone(),
                },
                TextSection {
                    value: "".to_string(),
                    style: style.clone(),
                },
            ],
            ..Default::default() // alignment
        },
        style: Style {
            position_type: PositionType::Absolute,
            position: Rect {
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                ..Default::default()
            },
            ..Default::default() // boy, LOTS of these
        },
        ..Default::default()
    }).insert(FPSCounter);
}

// marker struct for FPS counter
#[derive(Component)]
struct FPSCounter;

// again borrowed from bevymark example
fn update_fps_debug_system(
    diagnostics: Res<Diagnostics>,
    mut query: Query<&mut Text, With<FPSCounter>>,
) {
    if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(average) = fps.average() {
            for mut text in query.iter_mut() {
                text.sections[1].value = format!("{:.2}", average);
            }
        }
    }
}

fn setup_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    // Time to start re-typing everything from bevy/examples/2d/sprite_sheet.rs. well, we all start somewhere.

    // vv OK, so apparently asset_server.load() CAN infer the type of a handle for a receiving
    // binding without a type annotation, but only by looking *ahead* at where you consume the
    // handle! That's some rust magic. Anyway, in my case I'm still exploring so I guess I'll just
    // annotate.
    let texture_handle: Handle<Image> = asset_server.load("sprites/sPlayerRun_strip32.png");
    // vv AH ha, and here's the bit I would want some automation for. Should be easy lol.
    let texture_atlas = TextureAtlas::from_grid(texture_handle, Vec2::new(17.0, 24.0), 32, 1);
    let texture_atlas_handle = texture_atlases.add(texture_atlas);

    let mut camera_bundle = OrthographicCameraBundle::new_2d();
    camera_bundle.orthographic_projection.scale = 1.0/3.0;
    commands.spawn_bundle(camera_bundle).insert(MainCamera); // Oh, hmm, gonna want to move that to another system later.
    commands.spawn_bundle(UiCameraBundle::default());
    commands
        .spawn_bundle(SpriteSheetBundle {
            texture_atlas: texture_atlas_handle,
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0)),
            ..Default::default()
        })
        .insert(Timer::from_seconds(0.1, true))
        // ^^ 0.1 = inverse FPS. Could be way more ergonomic.
        .insert(Speed(120.0))
        .insert(Player);
}

// Marker struct for the player
#[derive(Component)]
struct Player;

// Marker struct for main camera
#[derive(Component)]
struct MainCamera;

// Speed in pixels... per... second?
#[derive(Component)]
struct Speed(f32);
