#![allow(clippy::type_complexity)] // it's just impossible

use crate::char_animation::*;
use crate::collision::*;
use crate::compass::*;
use crate::input::*;
use crate::movement::*;
use crate::render::*;
use bevy::{
    input::InputSystem,
    log::LogPlugin,
    math::Rect,
    prelude::*,
    render::{RenderApp, RenderStage},
    utils::{tracing, Duration},
};
use bevy_ecs_ldtk::prelude::*;
use bevy_inspector_egui::{
    Inspectable, InspectorPlugin, RegisterInspectable, WorldInspectorPlugin,
};
use bevy_spatial::RTreePlugin2D;
use std::collections::HashMap;
use std::io::Write;

mod char_animation;
mod collision;
mod compass;
mod goofy_time;
mod hellow;
mod input;
mod junk;
mod movement;
mod player_states;
mod render;

const PIXEL_SCALE: f32 = 1.0;

fn main() {
    let configured_default_plugins = DefaultPlugins
        .set(WindowPlugin {
            window: WindowDescriptor {
                present_mode: bevy::window::PresentMode::Fifo,
                cursor_visible: true,
                // mode: bevy::window::WindowMode::BorderlessFullscreen,
                // width: 1920.0,
                // height: 1080.0,
                ..default()
            },
            ..default()
        })
        .set(AssetPlugin {
            watch_for_changes: true,
            ..default()
        })
        .set(ImagePlugin::default_nearest())
        .set(LogPlugin {
            level: tracing::Level::INFO,
            filter: "wgpu=error".to_string(),
        });

    let mut app = App::new();
    app
        .add_plugins(configured_default_plugins)
        .add_plugin(CharAnimationPlugin)
        .add_plugin(TestCharAnimationPlugin)
        .add_plugin(LdtkPlugin)
        // DEBUG STUFF
        // .add_plugin(FrameTimeDiagnosticsPlugin)
        // .add_startup_system(junk::setup_fps_debug)
        // .add_system(junk::update_fps_debug_system)
        // .add_system(junk::debug_z_system)
        // .add_system(junk::tile_info_barfing_system)
        .insert_resource(DebugAssets::default())
        .add_startup_system(setup_debug_assets.before(setup_player))
        .add_system(spawn_collider_debugs)
        .insert_resource(DebugSettings::default())
        // INSPECTOR STUFF
        .add_plugin(WorldInspectorPlugin::new())
        .register_inspectable::<SubTransform>()
        .register_inspectable::<Speed>()
        .register_inspectable::<Walkbox>()
        .register_inspectable::<Hitbox>()
        .register_inspectable::<TopDownMatter>()
        .register_inspectable::<PhysicsSpaceOffset>()
        .add_plugin(InspectorPlugin::<DebugSettings>::new())
        .add_system(debug_walkboxes_system)
        // LDTK STUFF
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)
        // SPATIAL PARTITIONING STUFF
        .add_plugin(RTreePlugin2D::<Solid> { ..default() })
        // CAMERA
        .add_startup_system(setup_camera)
        // INPUT STUFF
        .add_system(connect_gamepads_system)
        .insert_resource(CurrentInputs::default())
        .add_system_to_stage(CoreStage::PreUpdate, accept_input_system.after(InputSystem))
        // BODY STUFF
        .add_system(shadow_stitcher_system)
        // PLAYER STUFF
        .add_startup_system(setup_player)
        .add_system(
            move_whole_pixel
                .label(Movers)
                .after(CharAnimationSystems)
                .after(MovePlanners)
        )
        .add_system(
            move_continuous_no_collision
                .label(Movers)
                .after(CharAnimationSystems)
                .after(MovePlanners)
        )
        .add_system(
            move_continuous_faceplant
                .label(Movers)
                .after(CharAnimationSystems)
                .after(MovePlanners)
        )
        .add_system(
            move_continuous_ray_test
                .label(Movers)
                .after(CharAnimationSystems)
                .after(MovePlanners)
        )
        .add_system_set(
            SystemSet::new()
                .label(MovePlanners)
                .with_system(player_free_plan_move.label(SpriteChangers))
                .with_system(player_roll_plan_move)
                .with_system(player_bonk_plan_move)
        )
        .add_system_set(
            SystemSet::new()
                .label(SpriteChangers)
                .before(MovePlanners)
                .with_system(player_roll_enter)
                .with_system(player_bonk_enter)
        )
        .add_system_to_stage(CoreStage::PostUpdate, player_roll_out)
        .add_system_to_stage(CoreStage::PostUpdate, player_free_out)
        .add_system_to_stage(CoreStage::PostUpdate, player_bonk_out)
        // .add_system(charanm_test_animate_system) // in CharAnimationPlugin or somethin'
        .add_system_set(
            SystemSet::new()
                .label(CameraMovers)
                .after(Movers)
                .with_system(camera_locked_system)
                .with_system(camera_lerp_system)
        )
        .add_system(snap_pixel_positions_system.after(CameraMovers))
        // OK BYE!!!
        ;

    if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            // SPACE STUFF
            .add_system_to_stage(
                RenderStage::Extract,
                extract_and_flatten_space_system.after(bevy::sprite::SpriteSystem::ExtractSprites),
            );
    }

    if std::env::args().any(|arg| &arg == "--graph") {
        // Write the debug dump to a file and exit. (not sure why it exits, though??? oh well!)
        let system_schedule = bevy_mod_debugdump::get_schedule(&mut app);
        let mut sched_file = std::fs::File::create("./schedule.dot").unwrap();
        sched_file.write_all(system_schedule.as_bytes()).unwrap();
    }

    app.run();
}

#[derive(Resource, Default, Reflect, Inspectable, PartialEq, Eq)]
pub struct DebugSettings {
    debug_walkboxes: bool,
    motion_kind: MotionKind,
    camera_kind: CameraKind,
}

#[derive(Resource, Reflect, Inspectable, Default, PartialEq, Eq)]
enum MotionKind {
    NoCollision,
    Faceplant,
    #[default]
    RayTest,
    WholePixel,
}

#[derive(Resource, Reflect, Inspectable, Default, PartialEq, Eq)]
enum CameraKind {
    #[default]
    Locked,
    Lerp,
}

#[derive(SystemLabel)]
pub struct MovePlanners;

#[derive(SystemLabel)]
pub struct Movers;

#[derive(SystemLabel)]
struct CameraMovers;

fn player_free_plan_move(
    inputs: Res<CurrentInputs>,
    mut player_q: Query<
        (
            &mut Motion,
            &mut Speed,
            &AnimationsMap,
            &mut CharAnimationState,
            &mut PlayerFree,
        ),
        With<Player>,
    >,
) {
    for (mut motion, mut speed, animations_map, mut animation_state, mut player_free) in
        player_q.iter_mut()
    {
        if player_free.just_started {
            speed.0 = Speed::RUN; // TODO hey what's the value here
            player_free.just_started = false;
        }

        let move_input = inputs.movement;

        if move_input.length() > 0.0 {
            // OK, we're running
            let run = animations_map.get("run").unwrap().clone();
            animation_state.change_animation(run, Playback::Loop);
        } else {
            // Chill
            let idle = animations_map.get("idle").unwrap().clone();
            animation_state.change_animation(idle, Playback::Loop);
        }

        // Publish movement intent
        let velocity = move_input * speed.0;
        motion.velocity += velocity;
        motion.face(move_input);

        // OK, that's movement. Are we doing anything else this frame? For example, an action?
        if inputs.actioning {
            // Right now there is only roll.
            player_free.transition = PlayerFreeTransition::Roll {
                direction: motion.facing,
            };
        }
    }
}

fn player_free_out(mut commands: Commands, player_q: Query<(Entity, &PlayerFree)>) {
    for (entity, player_free) in player_q.iter() {
        match &player_free.transition {
            PlayerFreeTransition::None => (),
            PlayerFreeTransition::Roll { direction } => {
                commands
                    .entity(entity)
                    .remove::<PlayerFree>()
                    .insert(PlayerRoll::new(*direction));
            },
        }
    }
}

fn player_bonk_enter(
    mut player_q: Query<
        (
            &mut Speed,
            &mut CharAnimationState,
            &mut Motion,
            &AnimationsMap,
        ),
        Added<PlayerBonk>,
    >,
) {
    for (mut speed, mut animation_state, mut motion, animations_map) in player_q.iter_mut() {
        speed.0 = Speed::BONK;
        let hurt = animations_map.get("hurt").unwrap().clone();
        animation_state.change_animation(hurt, Playback::Once);
        // single frame on this, so no add'l fussing.
        motion.remainder = Vec2::ZERO;
    }
}

fn player_bonk_plan_move(
    mut player_q: Query<
        (&mut Motion, &mut Speed, &mut PlayerBonk, &mut SubTransform),
        With<Player>,
    >,
    time: Res<Time>,
) {
    // Right. Basically same as roll, so much so that I clearly need to do some recombining.

    for (mut motion, speed, mut player_bonk, mut transform) in player_q.iter_mut() {
        // Contribute velocity
        let velocity = player_bonk.bonk_input * speed.0;
        motion.velocity += velocity;

        // Chip away at state duration
        player_bonk.timer.tick(time.delta());
        let progress = player_bonk.timer.percent();
        // ^^ ok to go backwards bc sin is symmetric. btw this should probably
        // be parabolic but shrug for now. Also BTW, progress will be exactly
        // 1.0 (and thus height_frac 0.0) if the timer finished, so we shouldn't
        // need a backstop against height drift here. *NARRATOR VOICE:* it was
        // actually -0.0, and that never came back to bite them.
        let height_frac = (progress * std::f32::consts::PI).sin();
        // and...  we're just manipulating Z directly instead of going through
        // the motion planning system. Sorry!! Maybe later.
        transform.translation.z = height_frac * PlayerBonk::HEIGHT;
    }
}

fn player_roll_enter(
    mut player_q: Query<
        (
            &mut Speed,
            &AnimationsMap,
            &mut CharAnimationState,
            &PlayerRoll,
            &mut Motion,
        ),
        Added<PlayerRoll>,
    >,
) {
    for (mut speed, animations_map, mut animation_state, player_roll, mut motion) in
        player_q.iter_mut()
    {
        speed.0 = Speed::ROLL;
        let roll = animations_map.get("roll").unwrap().clone();
        animation_state.change_animation(roll, Playback::Once);
        // Scale the animation to match the configured roll distance/speed:
        let roll_duration_millis = player_roll.timer.duration().as_millis() as u64;
        animation_state.set_total_run_time_to(roll_duration_millis);

        // Re-set the motion remainder because we're doing a different kind of motion now:
        motion.remainder = Vec2::ZERO; // HMM, actually not 100% sure about that. Well anyway!!
    }
}

fn player_roll_plan_move(
    mut player_q: Query<(&mut Motion, &Speed, &mut PlayerRoll)>,
    time: Res<Time>,
) {
    for (mut motion, speed, mut player_roll) in player_q.iter_mut() {
        // Contribute velocity
        let velocity = player_roll.roll_input * speed.0;
        motion.velocity += velocity;
        // Spend state duration
        player_roll.timer.tick(time.delta());
    }
}

fn player_roll_out(mut commands: Commands, mut player_q: Query<(Entity, &PlayerRoll, &Motion)>) {
    for (entity, player_roll, motion) in player_q.iter_mut() {
        // If roll animation finished uninterrupted, switch back to free.
        if player_roll.timer.finished() {
            commands
                .entity(entity)
                .remove::<PlayerRoll>()
                .insert(PlayerFree::new());
        } else if let Some(MotionResult { collided: true, .. }) = motion.result {
            // If we hit a wall, switch to bonk
            let opposite_direction = Vec2::X.angle_between(-1.0 * Vec2::from_angle(motion.facing));
            let bonk = PlayerBonk::roll(opposite_direction);
            commands.entity(entity).remove::<PlayerRoll>().insert(bonk);
        }
    }
}

/// Bonk state ends when it ends.
fn player_bonk_out(mut commands: Commands, player_q: Query<(Entity, &PlayerBonk)>) {
    for (entity, player_bonk) in player_q.iter() {
        if player_bonk.timer.finished() {
            commands
                .entity(entity)
                .remove::<PlayerBonk>()
                .insert(PlayerFree::new());
        }
    }
}

fn camera_lerp_system(
    time: Res<Time>,
    // time: Res<StaticTime>,
    // time: Res<SmoothedTime>,
    mut params: ParamSet<(
        Query<&SubTransform, With<Player>>,
        Query<&mut SubTransform, With<Camera>>,
    )>,
    debug_settings: Res<DebugSettings>,
) {
    if debug_settings.camera_kind != CameraKind::Lerp {
        return;
    }

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

fn camera_locked_system(
    mut params: ParamSet<(
        Query<&SubTransform, With<Player>>,
        Query<&mut SubTransform, With<Camera>>,
    )>,
    debug_settings: Res<DebugSettings>,
) {
    if debug_settings.camera_kind != CameraKind::Locked {
        return;
    }

    let player_pos = params.p0().single().translation;
    let mut camera_q = params.p1();
    let mut camera_tf = camera_q.single_mut();
    camera_tf.translation.x = player_pos.x;
    camera_tf.translation.y = player_pos.y;
}

// This no longer does anything, because we're now handling the sub-pixel stuff
// in the movement function -- though that's all still up in the air. Anyway,
// point being we always manipulate this alternate transform, and then this is
// the system that syncs it to real transform before the sync to global
// transform happens.
fn snap_pixel_positions_system(mut query: Query<(&SubTransform, &mut Transform)>) {
    // let global_scale = Vec3::new(PIXEL_SCALE, PIXEL_SCALE, 1.0);
    for (sub_tf, mut pixel_tf) in query.iter_mut() {
        // pixel_tf.translation = (global_scale * sub_tf.translation).floor();
        pixel_tf.translation = sub_tf.translation;
    }
}

fn setup_level(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        LdtkWorld,
        LdtkWorldBundle {
            ldtk_handle: asset_server.load("kittytown.ldtk"),
            transform: Transform::from_scale(Vec3::splat(PIXEL_SCALE)),
            ..Default::default()
        },
    ));
}

fn setup_camera(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scale = 1.0 / 3.0;
    commands.spawn((
        camera_bundle,
        SubTransform {
            translation: Vec3::new(0.0, 0.0, 999.0),
        },
        // ^^ hack: I looked up the Z coord on new_2D and fudged it so we won't accidentally round it to 1000.
    ));
}

fn setup_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    let run: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRun.aseprite");
    let idle: Handle<CharAnimation> = asset_server.load("sprites/sPlayer.aseprite");
    let hurt: Handle<CharAnimation> = asset_server.load("sprites/sPlayerHurt.aseprite");
    let roll: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRoll.aseprite");

    let initial_animation = idle.clone();

    // Okay, I'm going to be lazy here: hashmap w/ string literals. SORRY. I'll
    // come back to it later.
    let mut animations = AnimationsMap::default();
    animations.insert("run", run);
    animations.insert("idle", idle);
    animations.insert("hurt", hurt);
    animations.insert("roll", roll);

    // IT'S THE PLAYER, GIVE IT UP!!
    commands.spawn((
        PlayerBundle {
            // Remember who u are
            identity: Player,
            sprite_sheet: SpriteSheetBundle {
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0))
                    .with_scale(Vec3::splat(PIXEL_SCALE)),
                ..Default::default()
            },
            sub_transform: SubTransform {
                translation: Vec3::ZERO,
            },
            speed: Speed(Speed::RUN),
            walkbox: Walkbox(Rect::default()),
            // --- New animation system
            char_animation_state: CharAnimationState::new(
                initial_animation,
                Dir::E,
                Playback::Loop,
            ),
            motion: Motion::new(Vec2::ZERO),
            animations_map: animations,
        },
        // Initial gameplay state
        PlayerFree::new(),
        // Shadow marker
        HasShadow,
        // Draw-depth manager
        TopDownMatter::character(),
    ));
}

// Structs and crap!

#[derive(Bundle)]
struct PlayerBundle {
    identity: Player,
    sprite_sheet: SpriteSheetBundle,

    sub_transform: SubTransform,
    speed: Speed,
    walkbox: Walkbox,

    char_animation_state: CharAnimationState,
    motion: Motion,
    animations_map: AnimationsMap,
}

#[derive(Component, Deref, DerefMut, Default)]
pub struct AnimationsMap(HashMap<&'static str, Handle<CharAnimation>>);

/// Player state: freely able to move (but might be idling, idk)
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PlayerFree {
    pub just_started: bool,
    pub transition: PlayerFreeTransition,
}
impl PlayerFree {
    fn new() -> Self {
        PlayerFree {
            just_started: true,
            transition: PlayerFreeTransition::None,
        }
    }
}
#[derive(Clone)]
pub enum PlayerFreeTransition {
    None,
    Roll { direction: f32 },
}
impl Default for PlayerFreeTransition {
    fn default() -> Self {
        Self::None
    }
}
// ^^ That SparseSet storage might be useful or might not (I haven't profiled
// it), but this IS the specific use case it is suggested for (marker components
// that don't contain much data, don't get iterated over en masse on the
// regular, and get added and removed extremely frequently.)
/// Player state: rollin at the speed of sound
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PlayerRoll {
    /// The "player input" equivalent to use when planning motion; a length-1.0
    /// vector locked to the direction we were facing when starting our roll.
    pub roll_input: Vec2,
    pub timer: Timer,
}
impl PlayerRoll {
    const DISTANCE: f32 = 52.0;

    pub fn new(direction: f32) -> Self {
        let duration_secs = Self::DISTANCE / Speed::ROLL;
        PlayerRoll {
            roll_input: Vec2::from_angle(direction),
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        }
    }
}
/// Player state: bonkin'
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PlayerBonk {
    pub bonk_input: Vec2,
    pub timer: Timer,
}

impl PlayerBonk {
    const ROLL_BONK: f32 = 18.0;
    const HEIGHT: f32 = 8.0;
    pub fn roll(direction: f32) -> Self {
        Self::new(direction, Self::ROLL_BONK)
    }
    pub fn new(direction: f32, distance: f32) -> Self {
        let bonk_vector = Vec2::from_angle(direction);
        let duration_secs = distance / Speed::BONK;
        PlayerBonk {
            bonk_input: bonk_vector,
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        }
    }
}

/// Marker component for a spawned LdtkWorldBundle
#[derive(Component)]
pub struct LdtkWorld;

/// Marker component for the player
#[derive(Component)]
pub struct Player;

/// Speed in pixels... per... second?
#[derive(Component, Inspectable)]
pub struct Speed(f32);
impl Speed {
    const RUN: f32 = 120.0;
    const ROLL: f32 = 180.0;
    const BONK: f32 = 60.0;
}

/// Information about what the entity is doing, spatially speaking.
#[derive(Component)]
pub struct Motion {
    /// The direction the entity is currently facing, in radians. Tracked
    /// separately because it persists even when no motion is planned.
    pub facing: f32,
    /// The linear velocity for this frame, as determined by the entity's state and inputs.
    pub velocity: Vec2,
    pub remainder: Vec2,
    pub result: Option<MotionResult>,
}
impl Motion {
    pub fn new(motion: Vec2) -> Self {
        let mut thing = Self {
            facing: 0.0, // facing east on the unit circle
            velocity: Vec2::ZERO,
            remainder: Vec2::ZERO,
            result: None,
        };
        thing.face(motion);
        thing
    }

    pub fn face(&mut self, input: Vec2) {
        if input.length() > 0.0 {
            self.facing = Vec2::X.angle_between(input);
        }
    }
}

pub struct MotionResult {
    pub collided: bool,
    pub new_location: Vec2,
}

/// Additional transform component for things whose movements should be synced to hard pixel boundaries.
#[derive(Component, Inspectable)]
pub struct SubTransform {
    translation: Vec3,
}
