#![allow(clippy::type_complexity)] // it's just impossible

use crate::char_animation::*;
use crate::collision::*;
use crate::compass::*;
use crate::input::*;
use crate::movement::*;
use crate::phys_space::*;
use crate::render::*;
use bevy::{
    input::InputSystem,
    log::LogPlugin,
    math::Rect,
    prelude::*,
    render::{RenderApp, RenderStage},
    utils::tracing,
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
mod phys_space;
mod player_states;
mod render;
mod util_countup_timer;

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
        .register_inspectable::<PhysTransform>()
        .register_inspectable::<Speed>()
        .register_inspectable::<Walkbox>()
        .register_inspectable::<Hitbox>()
        .register_inspectable::<TopDownMatter>()
        .register_inspectable::<PhysOffset>()
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
        // .add_system_set(
        //     SystemSet::new()
        //         .label(MovePlanners)
        //         .with_system(player_free_plan_move.label(SpriteChangers))
        //         .with_system(player_roll_plan_move)
        //         .with_system(player_bonk_plan_move)
        // )
        // .add_system_set(
        //     SystemSet::new()
        //         .label(SpriteChangers)
        //         .before(MovePlanners)
        //         .with_system(player_roll_enter)
        //         .with_system(player_bonk_enter)
        // )
        // .add_system_to_stage(CoreStage::PostUpdate, player_roll_out)
        // .add_system_to_stage(CoreStage::PostUpdate, player_free_out)
        // .add_system_to_stage(CoreStage::PostUpdate, player_bonk_out)
        .add_system(propagate_inputs_to_player_state.before(handle_player_state_exits))
        .add_system(handle_player_state_exits.before(handle_player_state_entry))
        .add_system(handle_player_state_entry.label(SpriteChangers).before(MovePlanners))
        .add_system(plan_move.label(MovePlanners))
        .add_system(wall_collisions.after(Movers))
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

/// Hey, how much CAN I get away with processing at this point? I know I want to handle
/// walk/idle transitions here, but..... action button?
fn propagate_inputs_to_player_state(
    inputs: Res<CurrentInputs>,
    mut player_q: Query<(&mut PlayerStateMachine, &mut Motion)>,
) {
    for (mut machine, mut motion) in player_q.iter_mut() {
        // Moves -- ignored unless run or idle
        let move_input = inputs.movement;
        match machine.current() {
            PlayerState::Idle => {
                if move_input.length() > 0.0 {
                    machine.push_transition(PlayerState::Run);
                }
                motion.face(move_input); // sprite-relevant.
            },
            PlayerState::Run => {
                if move_input.length() == 0.0 {
                    machine.push_transition(PlayerState::Idle);
                }
                motion.face(move_input);
            },
            _ => (),
        }

        // Action button
        if inputs.actioning {
            // Right now there is only roll.
            match machine.current() {
                PlayerState::Idle | PlayerState::Run => {
                    machine.push_transition(PlayerState::roll(motion.facing));
                },
                _ => (),
            }
        }
    }
}

/// Case 1: interrupted states (something got requested as "next"). Case 2:
/// states coming to their natural conclusions (timers ran out). Either way,
/// two jobs: switch to the new state, and handle any state entry setup.
fn handle_player_state_exits(mut player_q: Query<&mut PlayerStateMachine>) {
    for mut machine in player_q.iter_mut() {
        // First, natural endings -- only queue these up if not pre-empted.
        if machine.next.is_none() {
            match machine.current() {
                PlayerState::Idle => (), // not timed
                PlayerState::Run => (),  // not timed
                PlayerState::Roll {
                    roll_input: _,
                    timer,
                } => {
                    if timer.finished() {
                        machine.push_transition(PlayerState::Idle);
                    }
                },
                PlayerState::Bonk {
                    bonk_input: _,
                    timer,
                } => {
                    if timer.finished() {
                        machine.push_transition(PlayerState::Idle);
                    }
                },
            }
        }

        // Then, move to next state, if any.
        machine.finish_transition();
    }
}

fn handle_player_state_entry(
    mut player_q: Query<(
        &mut PlayerStateMachine,
        &mut Speed,
        &mut CharAnimationState,
        &AnimationsMap,
        &mut Motion,
    )>,
) {
    for (mut machine, mut speed, mut animation_state, animations_map, mut motion) in
        player_q.iter_mut()
    {
        // only:
        if machine.just_changed {
            // Update sprite
            let mut set_anim = |name: &str, play: Playback| {
                let ani = animations_map.get(name).unwrap().clone();
                animation_state.change_animation(ani, play);
            };
            match machine.current() {
                PlayerState::Idle => set_anim("idle", Playback::Loop),
                PlayerState::Run => set_anim("run", Playback::Loop),
                PlayerState::Roll { timer, .. } => {
                    // little extra on this one, sets animation parameters based on gameplay effect
                    set_anim("roll", Playback::Once);
                    let roll_millis = timer.duration().as_millis() as u64;
                    animation_state.set_total_run_time_to(roll_millis);
                },
                PlayerState::Bonk { .. } => set_anim("hurt", Playback::Once),
            };

            // Update speed
            speed.0 = match machine.current() {
                PlayerState::Idle => 0.0,
                PlayerState::Run => Speed::RUN,
                PlayerState::Roll { .. } => Speed::ROLL,
                PlayerState::Bonk { .. } => Speed::BONK,
            };

            // Re-set motion remainder (not actually sure about this; also only applies to by-pixels move)
            motion.remainder = Vec2::ZERO;

            // Mark state entry as completed
            machine.state_entered();
        }
    }
}

/// Primarily mutates Motion, but for bonk state I have an unfortunate mutation
/// of SubTransform.z. Also, this is currently where I'm ticking state timers,
/// so that's mutable too. hmm.
fn plan_move(
    mut player_q: Query<(
        &mut PlayerStateMachine,
        &mut Motion,
        &Speed,
        &mut PhysTransform,
    )>,
    time: Res<Time>,
    inputs: Res<CurrentInputs>,
) {
    for (mut machine, mut motion, speed, mut transform) in player_q.iter_mut() {
        // Contribute velocity
        let input = match machine.current() {
            PlayerState::Idle => Vec2::ZERO,
            PlayerState::Run => inputs.movement,
            PlayerState::Roll { roll_input, .. } => *roll_input,
            PlayerState::Bonk { bonk_input, .. } => *bonk_input,
        };
        let velocity = input * speed.0;
        motion.velocity += velocity;

        // Spend state duration (TODO: put this elsewhere)
        // also: do fucky z-height hack for bonk
        match machine.current_mut() {
            PlayerState::Idle => (),
            PlayerState::Run => (),
            PlayerState::Roll { timer, .. } => {
                timer.tick(time.delta());
            },
            PlayerState::Bonk { timer, .. } => {
                timer.tick(time.delta());
                let progress = timer.percent();
                // ^^ ok to go backwards bc sin is symmetric. btw this should probably
                // be parabolic but shrug for now. Also BTW, progress will be exactly
                // 1.0 (and thus height_frac 0.0) if the timer finished, so we shouldn't
                // need a backstop against height drift here. *NARRATOR VOICE:* it was
                // actually -0.0, and that never came back to bite them.
                let height_frac = (progress * std::f32::consts::PI).sin();
                // and...  we're just manipulating Z directly instead of going through
                // the motion planning system. Sorry!! Maybe later.
                transform.translation.z = height_frac * PlayerState::BONK_HEIGHT;
            },
        }
    }
}

fn wall_collisions(mut player_q: Query<(&mut PlayerStateMachine, &Motion)>) {
    for (mut machine, motion) in player_q.iter_mut() {
        if let PlayerState::Roll { .. } = machine.current() {
            if let Some(MotionResult { collided: true, .. }) = motion.result {
                // We hit a wall, so bounce back:
                let opposite_direction = flip_angle(motion.facing);
                let next_state = PlayerState::bonk_from_roll(opposite_direction);
                machine.push_transition(next_state);
            }
        }
    }
}

fn camera_lerp_system(
    time: Res<Time>,
    // time: Res<StaticTime>,
    // time: Res<SmoothedTime>,
    mut params: ParamSet<(
        Query<&PhysTransform, With<Player>>,
        Query<&mut PhysTransform, With<Camera>>,
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
        Query<&PhysTransform, With<Player>>,
        Query<&mut PhysTransform, With<Camera>>,
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
fn snap_pixel_positions_system(mut query: Query<(&PhysTransform, &mut Transform)>) {
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
        PhysTransform {
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
            sub_transform: PhysTransform {
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
        PlayerStateMachine {
            current: PlayerState::Idle,
            next: None,
            just_changed: true,
        },
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

    sub_transform: PhysTransform,
    speed: Speed,
    walkbox: Walkbox,

    char_animation_state: CharAnimationState,
    motion: Motion,
    animations_map: AnimationsMap,
}

#[derive(Component, Deref, DerefMut, Default)]
pub struct AnimationsMap(HashMap<&'static str, Handle<CharAnimation>>);

#[derive(Component)]
pub struct PlayerStateMachine {
    // fields are private
    current: PlayerState,
    next: Option<PlayerState>,
    just_changed: bool,
}

impl PlayerStateMachine {
    fn push_transition(&mut self, next: PlayerState) {
        self.next = Some(next);
    }
    // fn has_transition(&self) -> bool {
    //     self.next.is_some()
    // }
    // fn peek_next(&self) -> Option<&PlayerState> {
    //     self.next.as_ref()
    // }
    fn current(&self) -> &PlayerState {
        &self.current
    }
    fn current_mut(&mut self) -> &mut PlayerState {
        &mut self.current
    }
    fn state_entered(&mut self) {
        self.just_changed = false;
    }
    fn finish_transition(&mut self) {
        if self.next.is_some() {
            let next = self.next.take().unwrap();
            self.current = next;
            self.just_changed = true;
        }
    }
}

#[derive(Clone)]
pub enum PlayerState {
    Idle,
    Run,
    Roll { roll_input: Vec2, timer: Timer },
    Bonk { bonk_input: Vec2, timer: Timer },
}

impl PlayerState {
    const ROLL_DISTANCE: f32 = 52.0;
    const BONK_FROM_ROLL_DISTANCE: f32 = 18.0;
    const BONK_HEIGHT: f32 = 8.0;
    const ROLL_SPEED: f32 = Speed::ROLL;
    const BONK_SPEED: f32 = Speed::BONK;

    fn roll(direction: f32) -> Self {
        let duration_secs = Self::ROLL_DISTANCE / Self::ROLL_SPEED;
        Self::Roll {
            roll_input: Vec2::from_angle(direction),
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        }
    }

    fn bonk(direction: f32, distance: f32) -> Self {
        let duration_secs = distance / Self::BONK_SPEED;
        Self::Bonk {
            bonk_input: Vec2::from_angle(direction),
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        }
    }

    fn bonk_from_roll(direction: f32) -> Self {
        Self::bonk(direction, Self::BONK_FROM_ROLL_DISTANCE)
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
