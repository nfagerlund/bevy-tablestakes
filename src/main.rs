#![allow(clippy::type_complexity)] // it's just impossible

use crate::camera::*;
use crate::char_animation::*;
use crate::collision::*;
use crate::compass::*;
use crate::input::*;
use crate::movement::*;
use crate::phys_space::*;
use crate::render::*;
use crate::space_lookup::RstarPlugin;
use bevy::{
    input::InputSystem,
    log::LogPlugin,
    math::Rect,
    prelude::*,
    render::RenderApp,
    utils::{tracing, Duration},
};
use bevy_ecs_ldtk::prelude::*;
use bevy_inspector_egui::quick::{ResourceInspectorPlugin, WorldInspectorPlugin};
use bevy_prng::Xoshiro256Plus;
use bevy_rand::prelude::*;
use rand::prelude::{IteratorRandom, Rng};
use std::collections::HashMap;
use std::io::Write;

mod junkbox;
mod toolbox;

mod camera;
mod char_animation;
mod collision;
mod compass;
mod goofy_time;
mod input;
mod movement;
mod phys_space;
mod player_states;
mod render;
mod space_lookup;

type GameRNG = GlobalEntropy<Xoshiro256Plus>;

fn main() {
    let configured_default_plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                cursor: bevy::window::Cursor::default(),
                present_mode: bevy::window::PresentMode::Fifo,
                mode: bevy::window::WindowMode::Windowed,
                position: bevy::window::WindowPosition::Automatic,
                resolution: bevy::window::WindowResolution::new(1280.0, 720.0),
                title: String::from("Kittyzone"),
                resizable: true,
                ..default()
            }),
            ..default()
        })
        .set(AssetPlugin {
            watch_for_changes: bevy::asset::ChangeWatcher::with_delay(Duration::from_millis(200)),
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
        .add_plugins(CharAnimationPlugin)
        .add_plugins(TestCharAnimationPlugin)
        .add_plugins(LdtkPlugin)
        .add_plugins(EntropyPlugin::<Xoshiro256Plus>::default())
        // DEBUG STUFF
        .insert_resource(DebugAssets::default())
        .add_systems(Startup, setup_debug_assets.before(setup_player))
        .add_systems(Update, spawn_collider_debugs)
        .insert_resource(DebugSettings::default())
        // INSPECTOR STUFF
        .add_plugins(WorldInspectorPlugin::new())
        .register_type::<PhysTransform>()
        .register_type::<PhysOffset>()
        .register_type::<Speed>()
        .register_type::<Walkbox>()
        .register_type::<Hitbox>()
        .register_type::<TopDownMatter>()
        .register_type::<Motion>()
        .add_plugins(ResourceInspectorPlugin::<DebugSettings>::new())
        .add_systems(Update, (
            debug_walkboxes_system,
            debug_hitboxes_system,
            debug_origins_system,
        ))
        // LDTK STUFF
        .add_systems(Startup, setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)
        // SPATIAL PARTITIONING STUFF
        .add_plugins(RstarPlugin::<Solid>::new())
        // CAMERA
        .add_systems(Startup, setup_camera)
        // INPUT STUFF
        .add_systems(Update, connect_gamepads_system)
        .insert_resource(CurrentInputs::default())
        .add_systems(PreUpdate, accept_input_system
            .after(InputSystem)
        )
        // SPRITE ASSET STUFF
        .insert_resource(AnimationsMap::default())
        .add_systems(Startup, load_sprite_assets)
        // BODY STUFF
        .add_systems(Update, shadow_stitcher_system)
        // ENEMY STUFF
        .add_systems(Startup, temp_setup_enemy.after(load_sprite_assets))
        .add_systems(Update, enemy_state_changes.in_set(SpriteChangers))
        .add_systems(Update, enemy_plan_move.in_set(MovePlanners))
        // PLAYER STUFF
        .add_systems(Startup, setup_player.after(load_sprite_assets))
        .configure_set(Update, Movers.after(CharAnimationSystems).after(MovePlanners))
        .configure_set(Update, MovePlanners.after(SpriteChangers))
        .configure_set(Update, CameraMovers.after(Movers))
        .add_systems(
            Update,
            (
                move_whole_pixel.run_if(motion_is(MotionKind::WholePixel)),
                move_continuous_no_collision.run_if(motion_is(MotionKind::NoCollision)),
                move_continuous_faceplant.run_if(motion_is(MotionKind::Faceplant)),
                move_continuous_ray_test.run_if(motion_is(MotionKind::RayTest)),
            ).in_set(Movers)
        )
        .add_systems(Update, player_state_read_inputs.before(player_state_changes))
        .add_systems(Update, player_state_changes.in_set(SpriteChangers).before(MovePlanners))
        .add_systems(Update, player_plan_move.in_set(MovePlanners))
        .add_systems(Update, player_queue_wall_bonk.after(Movers))
        .add_systems(
            Update,
            (
                camera_locked_system.run_if(camera_is(CameraKind::Locked)),
                camera_lerp_system.run_if(camera_is(CameraKind::Lerp)),
            ).in_set(CameraMovers)
        )
        // PHYSICS SPACE STUFF
        .add_systems(Update, add_new_phys_transforms.before(MovePlanners))
        .add_systems(Update, sync_phys_transforms.after(CameraMovers))
        // OK BYE!!!
        ;

    if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            // SPACE STUFF
            .add_systems(
                ExtractSchedule,
                extract_and_flatten_space_system.after(bevy::sprite::SpriteSystem::ExtractSprites),
            );
    }

    if std::env::args().any(|arg| &arg == "--graph") {
        // Write the debug dump to a file and exit. (not sure why it exits, though??? oh well!)
        let settings = bevy_mod_debugdump::schedule_graph::Settings::default();
        let system_schedule = bevy_mod_debugdump::schedule_graph_dot(&mut app, Update, &settings);
        let mut sched_file = std::fs::File::create("./schedule.dot").unwrap();
        sched_file.write_all(system_schedule.as_bytes()).unwrap();
    }

    app.run();
}

#[derive(Resource, Default, Reflect, PartialEq, Eq)]
pub struct DebugSettings {
    debug_walkboxes: bool,
    debug_origins: bool,
    debug_hitboxes: bool,
    motion_kind: MotionKind,
    camera_kind: CameraKind,
}

#[derive(Resource, Reflect, Default, PartialEq, Eq)]
enum MotionKind {
    NoCollision,
    Faceplant,
    #[default]
    RayTest,
    WholePixel,
}

#[derive(Resource, Reflect, Default, PartialEq, Eq)]
enum CameraKind {
    #[default]
    Locked,
    Lerp,
}

fn motion_is(kind: MotionKind) -> impl Fn(Res<DebugSettings>) -> bool {
    move |debugs: Res<DebugSettings>| debugs.motion_kind == kind
}
fn camera_is(kind: CameraKind) -> impl Fn(Res<DebugSettings>) -> bool {
    move |debugs: Res<DebugSettings>| debugs.camera_kind == kind
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MovePlanners;

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Movers;

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
struct CameraMovers;

/// Hey, how much CAN I get away with processing at this point? I know I want to handle
/// walk/idle transitions here, but..... action button?
fn player_state_read_inputs(
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

        // Attack button
        if inputs.attacking {
            match machine.current() {
                PlayerState::Idle | PlayerState::Run => {
                    machine.push_transition(PlayerState::attack());
                },
                _ => (),
            }
        }
    }
}

/// Near the start of every frame, check whether the player state machine is switching
/// states; if so, handle any setup and housekeeping to make the new state usable on the
/// current frame.
fn player_state_changes(
    mut player_q: Query<(
        &mut PlayerStateMachine,
        &mut StateTimer,
        &mut Speed,
        &mut CharAnimationState,
    )>,
    animations_map: Res<AnimationsMap>,
    time: Res<Time>,
) {
    for (mut machine, mut state_timer, mut speed, mut animation_state) in player_q.iter_mut() {
        // ZEROTH: if a state used up its time allotment last frame (without being interrupted),
        // this is where we queue up a transition to the next state.
        if let Some(ref timer) = state_timer.0 {
            if machine.next.is_none() && timer.finished() {
                match machine.current() {
                    PlayerState::Idle => (), // not timed
                    PlayerState::Run => (),  // not timed
                    PlayerState::Roll { .. } => machine.push_transition(PlayerState::Idle),
                    PlayerState::Bonk { .. } => machine.push_transition(PlayerState::Idle),
                    PlayerState::Attack => machine.push_transition(PlayerState::Idle),
                }
            }
        }

        // FIRST: change states, if it's time to.
        machine.finish_transition();

        // SECOND: if we changed states, do all our setup housekeeping for the new state.
        if machine.just_changed {
            // Set new Option<Timer>
            state_timer.0 = machine.current().timer();

            // Update sprite
            let mut set_anim = |name: &Ases, play: Playback| {
                if let Some(ani) = animations_map.get(name) {
                    animation_state.change_animation(ani.clone(), play);
                } else {
                    info!("Whoa oops, tried to set animation {:?} on player", name);
                }
            };
            match machine.current() {
                PlayerState::Idle => set_anim(&Ases::TkIdle, Playback::Loop),
                PlayerState::Run => set_anim(&Ases::TkRun, Playback::Loop),
                PlayerState::Roll { .. } => {
                    // little extra on this one, sets animation parameters based on gameplay effect
                    set_anim(&Ases::TkRoll, Playback::Once);
                    if let Some(ref mut timer) = state_timer.0 {
                        let roll_millis = timer.duration().as_millis() as u64;
                        animation_state.set_total_run_time_to(roll_millis);
                    } else {
                        warn!("no timer for roll state??");
                    }
                },
                PlayerState::Bonk { .. } => set_anim(&Ases::TkHurt, Playback::Once),
                PlayerState::Attack => {
                    set_anim(&Ases::TkSlash, Playback::Once);
                    if let Some(ref mut timer) = state_timer.0 {
                        let attack_millis = timer.duration().as_millis() as u64;
                        animation_state.set_total_run_time_to(attack_millis);
                    } else {
                        warn!("no timer for attack state??");
                    }
                },
            };

            // Update speed
            speed.0 = match machine.current() {
                PlayerState::Idle => 0.0,
                PlayerState::Run => Speed::RUN,
                PlayerState::Roll { .. } => Speed::ROLL,
                PlayerState::Bonk { .. } => Speed::BONK,
                PlayerState::Attack { .. } => 0.0,
            };

            // Mark state entry as completed
            machine.state_entered();
        }

        // THIRD: If the current state has a timer, tick it forward.
        if let Some(ref mut timer) = state_timer.0 {
            timer.tick(time.delta());
        }
    }
}

/// Compute player's velocity for the current frame.
/// Primarily mutates Motion, but for bonk state I have an unfortunate mutation
/// of PhysTransform.z.
fn player_plan_move(
    mut player_q: Query<(
        &PlayerStateMachine,
        &StateTimer,
        &mut Motion,
        &Speed,
        &mut PhysTransform,
    )>,
    inputs: Res<CurrentInputs>,
) {
    for (machine, state_timer, mut motion, speed, mut transform) in player_q.iter_mut() {
        // Contribute velocity
        let input = match machine.current() {
            PlayerState::Idle => Vec2::ZERO,
            PlayerState::Run => inputs.movement,
            PlayerState::Roll { roll_input, .. } => *roll_input,
            PlayerState::Bonk { bonk_input, .. } => *bonk_input,
            PlayerState::Attack { .. } => Vec2::ZERO,
        };
        let velocity = input * speed.0;
        motion.velocity += velocity;

        // do fucky z-height hack for bonk
        if let PlayerState::Bonk { .. } = machine.current() {
            if let Some(ref timer) = state_timer.0 {
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
            } else {
                warn!("yo, no timer for bonk state??")
            }
        }
    }
}

/// If player bonked into a wall, queue a state transition.
/// TODO: Generalize knockback. why should this be player-specific? Or bonk-specific?
fn player_queue_wall_bonk(mut player_q: Query<(&mut PlayerStateMachine, &Motion)>) {
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

fn setup_level(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        LdtkWorld,
        LdtkWorldBundle {
            ldtk_handle: asset_server.load("kittytown.ldtk"),
            transform: Transform::default(),
            ..Default::default()
        },
    ));
}

/// Name enum for ALL the sprites I'm using. 😵‍💫😽 I just want something type-checked instead
/// of a hashmap of strings, that's all. Keep it dumb.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum Ases {
    // Tk = Tutorial Kitty
    TkIdle,
    TkRun,
    TkHurt,
    TkRoll,
    TkSlash,
    SlimeIdle,
    SlimeAttack,
    SlimeHurt,
    SlimeDie,
}

/// Sets up a shared hashmap resource of loaded animated sprite assets.
fn load_sprite_assets(asset_server: Res<AssetServer>, mut animations: ResMut<AnimationsMap>) {
    // Tutorial Kitty
    animations.insert(
        Ases::TkRun,
        asset_server.load("sprites/sPlayerRun.aseprite"),
    );
    animations.insert(Ases::TkIdle, asset_server.load("sprites/sPlayer.aseprite"));
    animations.insert(
        Ases::TkHurt,
        asset_server.load("sprites/sPlayerHurt.aseprite"),
    );
    animations.insert(
        Ases::TkRoll,
        asset_server.load("sprites/sPlayerRoll.aseprite"),
    );
    animations.insert(
        Ases::TkSlash,
        asset_server.load("sprites/sPlayerAttackSlash.aseprite"),
    );

    // Tutorial Slime
    animations.insert(
        Ases::SlimeIdle,
        asset_server.load("sprites/sSlime.aseprite"),
    );
    animations.insert(
        Ases::SlimeAttack,
        asset_server.load("sprites/sSlimeAttack.aseprite"),
    );
    animations.insert(
        Ases::SlimeHurt,
        asset_server.load("sprites/sSlimeHurt.aseprite"),
    );
    animations.insert(
        Ases::SlimeDie,
        asset_server.load("sprites/sSlimeDie.aseprite"),
    );
}

fn enemy_state_changes(
    mut query: Query<(
        &mut EnemyStateMachine,
        &Speed,
        &mut CharAnimationState,
        &Patrol,
        &PhysTransform,
    )>,
    time: Res<Time>,
    mut rng: ResMut<GameRNG>,
    animations_map: Res<AnimationsMap>,
) {
    // Going in serial, because I'm using a global RNG still (instead of forking it to each enemy)
    for (mut machine, speed, mut anim, patrol, transform) in query.iter_mut() {
        // ZEROTH: if a state spent its timer, queue a transition.
        match machine.current() {
            EnemyState::Idle { timer } => {
                if timer.finished() {
                    // Decide where we're patrolling to next
                    if let Patrol::Patch { home, radius } = patrol {
                        let angle: f32 =
                            rng.gen_range(-(std::f32::consts::PI)..=std::f32::consts::PI);
                        let distance: f32 = rng.gen_range(0.0..*radius);
                        let dest = *home + Vec2::from_angle(angle) * distance;
                        let displacement = dest - transform.translation.truncate();
                        let input = displacement.normalize_or_zero();
                        let duration_secs = displacement.length() / speed.0;
                        machine.push_transition(EnemyState::Patrol {
                            patrol_input: input,
                            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
                        });
                    }
                }
            },
            EnemyState::Patrol { timer, .. } => {
                if timer.finished() {
                    machine.push_transition(EnemyState::Idle {
                        timer: Timer::from_seconds(2.0, TimerMode::Once),
                    });
                }
            },
            EnemyState::Chase => todo!(),
            EnemyState::Attack => todo!(),
            EnemyState::Hurt => todo!(),
            EnemyState::Dying => todo!(),
        }

        // FIRST: change states, if it's time to.
        machine.finish_transition();

        // SECOND: if we changed states, do all our setup housekeeping for the new state.
        if machine.just_changed {
            // Update sprite
            let mut set_anim = |name: &Ases, play: Playback| {
                if let Some(ani) = animations_map.get(name) {
                    anim.change_animation(ani.clone(), play);
                } else {
                    info!("Whoa oops, tried to set animation {:?} on enemy", name);
                }
            };
            match machine.current() {
                EnemyState::Idle { .. } => set_anim(&Ases::SlimeIdle, Playback::Loop),
                EnemyState::Patrol { .. } => set_anim(&Ases::SlimeAttack, Playback::Loop),
                EnemyState::Chase => todo!(),
                EnemyState::Attack => todo!(),
                EnemyState::Hurt => todo!(),
                EnemyState::Dying => todo!(),
            }

            // Mark state entry as completed
            machine.state_entered();
        }

        // Finally: if the current state has a timer, tick it.
        match machine.current_mut() {
            EnemyState::Idle { timer } => {
                timer.tick(time.delta());
            },
            EnemyState::Patrol { timer, .. } => {
                timer.tick(time.delta());
            },
            EnemyState::Chase => (),
            EnemyState::Attack => (),
            EnemyState::Hurt => (),
            EnemyState::Dying => (),
        };
    }
}

fn enemy_plan_move(mut query: Query<(&mut Motion, &Speed, &EnemyStateMachine)>) {
    query.for_each_mut(|(mut motion, speed, machine)| {
        let input = match machine.current() {
            EnemyState::Idle { .. } => Vec2::ZERO,
            EnemyState::Patrol { patrol_input, .. } => *patrol_input,
            EnemyState::Chase => Vec2::ZERO,  // TODO
            EnemyState::Attack => Vec2::ZERO, // TODO,
            EnemyState::Hurt => Vec2::ZERO,   // TODO,
            EnemyState::Dying => Vec2::ZERO,  // TODO,
        };
        let velocity = input * speed.0;
        motion.velocity += velocity;
        motion.face(input);
    })
}

// Obviously this is wack, and we should be spawning from ldtk entities, but bear with me here.
fn temp_setup_enemy(mut commands: Commands, animations: Res<AnimationsMap>) {
    let initial_animation = animations.get(&Ases::SlimeIdle).unwrap().clone();
    let whence = Vec3::new(220., 200., 0.); // empirically 🤷🏽

    commands.spawn((
        EnemyBundle {
            identity: Enemy,
            name: Name::new("Sloom"),
            state_machine: EnemyStateMachine {
                current: EnemyState::default(),
                next: None,
                just_changed: true,
            },
            sprite_sheet: SpriteSheetBundle::default(), // Oh huh wow, I took over all that stuff.
            char_animation_state: CharAnimationState::new(
                initial_animation,
                Dir::E,
                Playback::Loop,
            ),
            phys_transform: PhysTransform {
                translation: whence,
            },
            phys_offset: PhysOffset(Vec2::ZERO),
            walkbox: Walkbox(Rect::default()),
            hitbox: Hitbox(None),
            shadow: HasShadow,
            top_down_matter: TopDownMatter::character(),
            speed: Speed(Speed::RUN), // ???
            motion: Motion::new(Vec2::ZERO),
        },
        Patrol::Patch {
            home: whence.truncate(),
            radius: 140.0,
        },
    ));
}

fn setup_player(mut commands: Commands, animations: Res<AnimationsMap>) {
    let initial_animation = animations.get(&Ases::TkIdle).unwrap().clone();

    // IT'S THE PLAYER, GIVE IT UP!!
    commands.spawn((PlayerBundle {
        // Remember who u are
        identity: Player,
        sprite_sheet: SpriteSheetBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0)),
            ..Default::default()
        },
        phys_transform: PhysTransform {
            translation: Vec3::ZERO,
        },
        phys_offset: PhysOffset(Vec2::ZERO),
        speed: Speed(Speed::RUN),
        walkbox: Walkbox(Rect::default()),
        hitbox: Hitbox(None),
        // --- New animation system
        char_animation_state: CharAnimationState::new(initial_animation, Dir::E, Playback::Loop),
        motion: Motion::new(Vec2::ZERO),
        // Initial gameplay state
        state_machine: PlayerStateMachine {
            current: PlayerState::Idle,
            next: None,
            just_changed: true,
        },
        state_timer: StateTimer::default(),
        // Shadow marker
        shadow: HasShadow,
        // Draw-depth manager
        top_down_matter: TopDownMatter::character(),
        // Inspector?
        name: Name::new("Kittybuddy"),
    },));
}

// Structs and crap!

/// Marker component for enemies
#[derive(Component)]
struct Enemy;

type EnemyStateMachine = EntityStateMachine<EnemyState>;

#[derive(Clone)]
enum EnemyState {
    Idle { timer: Timer },
    Patrol { patrol_input: Vec2, timer: Timer },
    Chase,
    Attack,
    Hurt,
    Dying,
}

impl Default for EnemyState {
    fn default() -> Self {
        Self::Idle {
            timer: Timer::from_seconds(3.0, TimerMode::Once),
        }
    }
}

#[derive(Component)]
enum Patrol {
    Patch { home: Vec2, radius: f32 },
    Shush, // leave me alone about my irrefutable if lets, man
}

#[derive(Bundle)]
struct EnemyBundle {
    identity: Enemy,
    name: Name,
    state_machine: EnemyStateMachine,

    // .......oh nice, everything below here is same as player. Ripe for future consolidation!
    sprite_sheet: SpriteSheetBundle,
    char_animation_state: CharAnimationState,

    phys_transform: PhysTransform,
    phys_offset: PhysOffset,

    walkbox: Walkbox,
    hitbox: Hitbox,

    shadow: HasShadow,
    top_down_matter: TopDownMatter,

    speed: Speed,
    motion: Motion,
}

#[derive(Bundle)]
struct PlayerBundle {
    identity: Player,
    name: Name,
    state_machine: PlayerStateMachine,
    state_timer: StateTimer,

    sprite_sheet: SpriteSheetBundle,
    char_animation_state: CharAnimationState,

    phys_transform: PhysTransform,
    phys_offset: PhysOffset,

    walkbox: Walkbox,
    hitbox: Hitbox,

    shadow: HasShadow,
    top_down_matter: TopDownMatter,

    speed: Speed,
    motion: Motion,
}

#[derive(Resource, Deref, DerefMut, Default)]
pub struct AnimationsMap(HashMap<Ases, Handle<CharAnimation>>);

type PlayerStateMachine = EntityStateMachine<PlayerState>;

#[derive(Component)]
pub struct EntityStateMachine<T>
where
    T: Clone,
{
    // fields are private
    current: T,
    next: Option<T>,
    just_changed: bool,
}

impl<T: Clone> EntityStateMachine<T> {
    fn push_transition(&mut self, next: T) {
        self.next = Some(next);
    }
    // fn has_transition(&self) -> bool {
    //     self.next.is_some()
    // }
    // fn peek_next(&self) -> Option<&PlayerState> {
    //     self.next.as_ref()
    // }
    fn current(&self) -> &T {
        &self.current
    }
    fn current_mut(&mut self) -> &mut T {
        &mut self.current
    }
    fn state_entered(&mut self) {
        self.just_changed = false;
    }
    fn finish_transition(&mut self) {
        if let Some(next) = self.next.take() {
            self.current = next;
            self.just_changed = true;
        }
    }
}

#[derive(Component, Reflect, Default)]
pub struct StateTimer(pub Option<Timer>);

#[derive(Clone)]
pub enum PlayerState {
    Idle,
    Run,
    Roll { roll_input: Vec2 },
    Bonk { bonk_input: Vec2, distance: f32 },
    Attack,
}

impl PlayerState {
    const ROLL_DISTANCE: f32 = 52.0;
    const BONK_FROM_ROLL_DISTANCE: f32 = 18.0;
    const BONK_HEIGHT: f32 = 8.0;
    const ROLL_SPEED: f32 = Speed::ROLL;
    const BONK_SPEED: f32 = Speed::BONK;
    const ATTACK_DURATION_MS: u64 = 400;

    fn timer(&self) -> Option<Timer> {
        match self {
            PlayerState::Idle => None,
            PlayerState::Run => None,
            PlayerState::Roll { .. } => {
                let duration_secs = Self::ROLL_DISTANCE / Self::ROLL_SPEED;
                Some(Timer::from_seconds(duration_secs, TimerMode::Once))
            },
            PlayerState::Bonk { distance, .. } => {
                let duration_secs = distance / Self::BONK_SPEED;
                Some(Timer::from_seconds(duration_secs, TimerMode::Once))
            },
            PlayerState::Attack => Some(Timer::new(
                Duration::from_millis(Self::ATTACK_DURATION_MS),
                TimerMode::Once,
            )),
        }
    }

    // TODO: I'm scaling this one for now anyway, but, it'd be good to learn the length of a state
    // based on its sprite asset, so it can be *dictated* by the source file but not *managed*
    // by the animation system. ...Cache it with a startup system?
    fn attack() -> Self {
        Self::Attack
    }

    fn roll(direction: f32) -> Self {
        Self::Roll {
            roll_input: Vec2::from_angle(direction),
        }
    }

    fn bonk(direction: f32, distance: f32) -> Self {
        Self::Bonk {
            bonk_input: Vec2::from_angle(direction),
            distance,
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

/// Speed in pixels... per... second? This is used for _planning_ movement; once
/// the entity knows what it's trying to do this frame, it gets reduced to an
/// absolute velocity vector, in the Motion struct.
#[derive(Component, Reflect)]
pub struct Speed(f32);
impl Speed {
    const RUN: f32 = 60.0;
    const ROLL: f32 = 180.0;
    const BONK: f32 = 60.0;
}
