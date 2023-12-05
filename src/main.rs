#![allow(clippy::type_complexity)] // it's just impossible

use crate::{
    assets_setup::*, behaviors::*, camera::*, char_animation::*, collision::*, compass::*,
    debug_settings::*, entity_states::*, input::*, movement::*, phys_space::*, render::*,
    sounds::*, space_lookup::RstarPlugin,
};
use bevy::{
    // ecs::schedule::{LogLevel, ScheduleBuildSettings},
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
use std::io::Write;

mod assets_setup;
mod behaviors;
mod camera;
mod char_animation;
mod collision;
mod compass;
mod debug_settings;
mod entity_states;
mod goofy_time;
mod input;
mod junkbox;
mod movement;
mod phys_space;
mod render;
mod sounds;
mod space_lookup;
mod toolbox;

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
        // Ambiguity detection: gotta do this for each schedule of interest
        // But: I can't really do it until 0.12 gets the SceneSpawnerSystem out of my way!
        // .edit_schedule(Update, |schedule| {
        //     schedule.set_build_settings(ScheduleBuildSettings {
        //         ambiguity_detection: LogLevel::Warn,
        //         ..default()
        //     });
        // })
        .add_plugins(CharAnimationPlugin)
        .add_plugins(TestCharAnimationPlugin)
        .add_plugins(LdtkPlugin)
        .add_plugins(EntropyPlugin::<Xoshiro256Plus>::default())
        // DEBUG STUFF
        .insert_resource(DebugAssets::default())
        .add_systems(Startup, setup_debug_assets.before(setup_player))
        .add_systems(Update, spawn_collider_debugs)
        .insert_resource(DebugSettings::default())
        .insert_resource(NumbersSettings::default())
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
        .add_plugins(ResourceInspectorPlugin::<NumbersSettings>::new())
        .add_systems(Update, (
            debug_walkboxes_system,
            debug_hitboxes_system,
            debug_origins_system,
        ))
        // LDTK STUFF
        .add_systems(Startup, setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)
        .register_ldtk_int_cell_for_layer::<Wall>("TerrainKind", 3)
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
        // SOUND STUFF
        .add_systems(Startup, load_sound_effects)
        .add_systems(Update, sounds_thumps)
        // BODY STUFF
        .add_systems(Update, shadow_stitcher_system)
        // BEHAVIOR STUFF
        .add_plugins(BehaviorEventsPlugin)
        // ENEMY STUFF
        .add_systems(Startup, temp_setup_enemy.after(load_sprite_assets))
        .add_systems(
            Update,
            (
                enemy_state_read_events,
                enemy_state_changes
            ).chain().in_set(SpriteChangers))
        .add_systems(Update, acquire_aggro.after(Movers).after(CameraMovers))
        // PLAYER STUFF
        .add_event::<Landed>()
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
            ).in_set(Movers).ambiguous_with(Movers).before(move_z_axis)
        )
        .add_systems(Update, move_z_axis.in_set(Movers))
        .add_systems(
            Update,
            (
                player_state_read_inputs,
                player_state_read_events,
                player_state_changes,
                apply_deferred
            ).chain().in_set(SpriteChangers).before(MovePlanners)
        )
        .add_systems(
            Update,
            (
                mobile_free_velocity,
                mobile_fixed_velocity,
                launch_and_fall,
                mobile_chase_entity,
            ).in_set(MovePlanners),
        )
        .add_systems(Update, player_queue_wall_bonk.after(Movers))
        .add_systems(
            Update,
            (
                camera_locked_system.run_if(camera_is(CameraKind::Locked)),
                camera_lerp_system.run_if(camera_is(CameraKind::Lerp)),
            ).in_set(CameraMovers).ambiguous_with(CameraMovers)
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

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MovePlanners;

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Movers;

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
struct CameraMovers;

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

// Obviously this is wack, and we should be spawning from ldtk entities, but bear with me here.
fn temp_setup_enemy(mut commands: Commands, animations: Res<AnimationsMap>) {
    let initial_animation = animations.get(&Ases::SlimeIdle).unwrap().clone();
    let whence = Vec3::new(220., 200., 0.); // empirically 🤷🏽

    commands.spawn((EnemyBundle {
        identity: Enemy,
        name: Name::new("Sloom"),
        state_machine: EnemyStateMachine::new(EnemyState::default()),
        state_timer: StateTimer::default(),
        sprite_sheet: SpriteSheetBundle::default(), // Oh huh wow, I took over all that stuff.
        char_animation_state: CharAnimationState::new(initial_animation, Dir::E, Playback::Loop),
        phys_transform: PhysTransform {
            translation: whence,
        },
        phys_offset: PhysOffset(Vec2::ZERO),
        walkbox: Walkbox(Rect::default()),
        hitbox: Hitbox(None),
        shadow: HasShadow,
        top_down_matter: TopDownMatter::character(),
        speed: Speed(Speed::ENEMY_RUN), // ???
        motion: Motion::new(Vec2::ZERO),

        patrol: PatrolArea::Patch {
            home: whence.truncate(),
            radius: 140.0,
        },
    },));
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
        state_machine: PlayerStateMachine::new(PlayerState::Idle),
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

#[derive(Bundle)]
struct EnemyBundle {
    identity: Enemy,
    name: Name,
    state_machine: EnemyStateMachine,
    state_timer: StateTimer,

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

    patrol: PatrolArea,
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

/// Marker component for a spawned LdtkWorldBundle
#[derive(Component)]
pub struct LdtkWorld;

/// Marker component for the player
#[derive(Component)]
pub struct Player;
