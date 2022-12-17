use bevy::{
    prelude::*,
    ecs::{
        component,
        system::{Command, Insert, Remove}
    },
    input::InputSystem,
    log::{info, LogPlugin},
    utils::{
        Duration,
        tracing,
    },
    math::Rect, sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};
use bevy_ecs_ldtk::prelude::*;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
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
use crate::char_animation::*;
use crate::compass::*;
// use crate::goofy_time::*;

mod hellow;
mod junk;
mod input;
mod char_animation;
mod compass;
mod player_states;
mod goofy_time;

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

    App::new()
        .add_plugins(configured_default_plugins)
        .add_plugin(CharAnimationPlugin)
        .add_plugin(TestCharAnimationPlugin)
        .add_plugin(LdtkPlugin)

        // DEBUG STUFF
        // .add_plugin(FrameTimeDiagnosticsPlugin)
        // .add_startup_system(junk::setup_fps_debug)
        // .add_system(junk::update_fps_debug_system)
        // .add_system(junk::debug_z_system)
        .add_system(tile_info_barfing_system)
        .insert_resource(DebugAssets::default())
        .add_startup_system(setup_debug_assets.before(setup_player))
        .add_system(spawn_wall_tile_collider_debugs)

        // INSPECTOR STUFF
        .add_plugin(WorldInspectorPlugin::new())
        .register_inspectable::<SubTransform>()
        .register_inspectable::<Speed>()
        .register_inspectable::<Walkbox>()
        .register_inspectable::<Hitbox>()
        .add_plugin(InspectorPlugin::<DebugColliders>::new())
        .add_system(debug_walkboxes_system)


        // LDTK STUFF
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)

        // CAMERA
        .add_startup_system(setup_camera)

        // INPUT STUFF
        .add_system(connect_gamepads_system)
        .insert_resource(CurrentInputs::default())
        .add_system_to_stage(CoreStage::PreUpdate, accept_input_system.after(InputSystem))

        // PLAYER STUFF
        .add_startup_system(setup_player)
        .add_system(planned_move_system)
        .add_system(player_free_plan_move.before(planned_move_system))
        .add_system(player_roll_plan_move.before(planned_move_system))
        .add_system_to_stage(CoreStage::PostUpdate, player_roll_out)
        .add_system_to_stage(CoreStage::PostUpdate, player_free_out)
        // .add_system(charanm_test_animate_system) // in CharAnimationPlugin or somethin'
        .add_system(dumb_move_camera_system.after(planned_move_system))
        .add_system(snap_pixel_positions_system.after(dumb_move_camera_system))

        // OK BYE!!!
        .run();
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

fn player_free_plan_move(
    inputs: Res<CurrentInputs>,
    time: Res<Time>,
    mut player_q: Query<
        (
            &mut Motion,
            &mut Speed,
            &AnimationsMap,
            &mut CharAnimationState,
            &mut PlayerFree,
        ),
        With<Player>
    >,
) {
    for (
        mut motion,
        mut speed,
        animations_map,
        mut animation_state,
        mut player_free,
    ) in player_q.iter_mut() {
        if player_free.just_started {
            speed.0 = Speed::RUN; // TODO hey what's the value here
            player_free.just_started = false;
        }

        let delta = time.delta_seconds();
        let move_input = inputs.movement;

        if move_input.length() > 0.0 {
            // OK, we're running
            let run = animations_map.get("run").unwrap().clone();
            animation_state.change_animation(run);
        } else {
            // Chill
            let idle = animations_map.get("idle").unwrap().clone();
            animation_state.change_animation(idle);
        }

        let raw_movement_intent = move_input * speed.0 * delta;

        // Publish movement intent
        motion.update_plan(raw_movement_intent);

        // OK, that's movement. Are we doing anything else this frame? For example, an action?
        if inputs.actioning {
            // Right now there is only roll.
            player_free.transition = PlayerFreeTransition::Roll { direction: motion.direction };
        }
    }
}

fn player_free_out(
    mut commands: Commands,
    player_q: Query<(Entity, &PlayerFree)>,
) {
    for (entity, player_roll) in player_q.iter() {
        match &player_roll.transition {
            PlayerFreeTransition::None => (),
            PlayerFreeTransition::Roll { direction } => {
                commands.entity(entity)
                    .remove::<PlayerFree>()
                    .insert(PlayerRoll::new(*direction));
            },
        }
    }
}

/// Shared system for Moving Crap Around. Consumes a planned movement from
/// Motion component, updates direction on same as needed, writes result to...
fn planned_move_system(
    mut mover_q: Query<(&mut SubTransform, &mut Motion, &Walkbox), With<Player>>,
    solids_q: Query<(&GlobalTransform, &Walkbox), With<Solid>>,
) {
    let solids: Vec<AbsBBox> = solids_q.iter().map(|(global_transform, walkbox)| {
        // TODO: This can't handle solids that move, because GlobalTransform
        // lags by one frame. I don't have a solution for this yet! Treat them
        // separately? Manually sync frames of reference for everything?
        // Aggressively early-update the GlobalTransform of anything mobile
        // right after it moves? Anyway, for immobile walls we need to do *this*
        // because as of bevy_ecs_ldtk 0.5 / bevy_ecs_tilemap 0.8+, tile
        // entities are offset from (0,0) by a half a tile in both axes in order
        // to make the bottom left corner of the first tile render at (0,0).
        let origin = global_transform.translation().truncate();
        AbsBBox::from_rect(walkbox.0, origin)
    }).collect();

    for (
        mut transform,
        mut motion,
        walkbox,
    ) in mover_q.iter_mut() {
        let raw_movement_intent = motion.planned;
        motion.planned = Vec2::ZERO; // TODO should probably have this be an Option -> .take()


        // If we're not moving, stop running and bail. Right now I'm not doing
        // change detection, so I can just do an unconditional hard assign w/ those
        // handles...
        if raw_movement_intent.length() == 0.0 {
            // Don't hold onto sub-pixel remainders from previous move sequences once we stop
            motion.remainder = Vec2::ZERO;
            // No result
            motion.result = None;
            // Direction unchanged.
        } else {
            // Ok go for it!!

            let mut location = transform.translation.truncate();
            let mut collided = false;
            // Bring in any remainder
            let movement_intent = raw_movement_intent + motion.remainder;
            let move_pixels = movement_intent.round();
            let remainder = movement_intent - move_pixels;

            let mut move_x = move_pixels.x;
            let sign_x = move_x.signum();
            while move_x != 0. {
                let next_location = location + Vec2::new(sign_x, 0.0);
                let next_box = AbsBBox::from_rect(walkbox.0, next_location);
                if let None = solids.iter().find(|s| s.collide(next_box)) {
                    location.x += sign_x;
                    move_x -= sign_x;
                } else {
                    // Hit a wall
                    collided = true;
                    break;
                }
            }
            let mut move_y = move_pixels.y;
            let sign_y = move_y.signum();
            while move_y != 0. {
                let next_origin = location + Vec2::new(0.0, sign_y);
                let next_box = AbsBBox::from_rect(walkbox.0, next_origin);
                if let None = solids.iter().find(|s| s.collide(next_box)) {
                    location.y += sign_y;
                    move_y -= sign_y;
                } else {
                    // Hit a wall
                    collided = true;
                    break;
                }
            }

            // Commit it
            transform.translation.x = location.x;
            transform.translation.y = location.y;
            motion.remainder = remainder;
            motion.result = Some(MotionResult {
                collided,
                new_location: location,
            });
        }
    }

}

fn player_roll_plan_move(
    mut player_q: Query<
        (&mut Motion, &mut Speed, &AnimationsMap, &mut CharAnimationState, &mut PlayerRoll),
        With<Player>
    >,
    time: Res<Time>,
) {
    // OK great, so... we just need to plan our motion and encode it in the
    // Motion struct, and oh wait, also mess with the animation timings.
    // I guess first things first:

    for (mut motion, mut speed, animations_map, mut animation_state, mut player_roll) in player_q.iter_mut() {
        if player_roll.just_started {
            player_roll.just_started = false;
            speed.0 = Speed::ROLL;
            let roll = animations_map.get("roll").unwrap().clone();
            animation_state.change_animation(roll);
            // Scale the animation to match the configured roll distance/speed:
            let roll_duration_millis = (player_roll.distance_remaining / speed.0 * 1000.0) as u64;
            animation_state.set_total_run_time_to(roll_duration_millis);

            // Re-set the motion remainder because we're doing a different kind of motion now:
            motion.remainder = Vec2::ZERO; // HMM, actually not 100% sure about that. Well anyway!!
        }

        let delta = time.delta_seconds();
        let raw_movement_intent = player_roll.roll_input * speed.0 * delta;
        let movement_intent = raw_movement_intent.clamp_length_max(player_roll.distance_remaining);

        motion.update_plan(movement_intent);
        player_roll.distance_remaining -= movement_intent.length();
        // ^^ You know, it occurs to me we could chip away at a long vector
        // instead. Not actually sure which is nicer yet!

        if player_roll.distance_remaining <= 0.0 {
            // Roll came to natural end and we want to transition to free.
            // Bonk transition has to shake out post-move in another system, but for now:
            player_roll.transition = PlayerRollTransition::Free;
        }
    }
}

fn player_roll_out(
    mut commands: Commands,
    player_q: Query<(Entity, &PlayerRoll), With<Player>>,
) {
    for (entity, player_roll) in player_q.iter() {
        match &player_roll.transition {
            PlayerRollTransition::None => (),
            PlayerRollTransition::Bonk => (),
            PlayerRollTransition::Free => {
                commands.entity(entity)
                    .remove::<PlayerRoll>()
                    .insert(PlayerFree::new());
            },
        }
    }
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

// This no longer does anything, because we're now handling the sub-pixel stuff
// in the movement function -- though that's all still up in the air. Anyway,
// point being we always manipulate this alternate transform, and then this is
// the system that syncs it to real transform before the sync to global
// transform happens.
fn snap_pixel_positions_system(
    mut query: Query<(&SubTransform, &mut Transform)>,
) {
    // let global_scale = Vec3::new(PIXEL_SCALE, PIXEL_SCALE, 1.0);
    for (sub_tf, mut pixel_tf) in query.iter_mut() {
        // pixel_tf.translation = (global_scale * sub_tf.translation).floor();
        pixel_tf.translation = sub_tf.translation;
    }
}

fn setup_debug_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut debug_assets: ResMut<DebugAssets>,
) {
    let walkbox_mesh = meshes.add(Mesh::from(shape::Quad::default()));
    let walkbox_material = materials.add(ColorMaterial::from(Color::rgba(0.5, 0.0, 0.5, 0.6)));
    debug_assets.insert("walkbox_mesh".to_string(), walkbox_mesh.clone_untyped());
    debug_assets.insert("walkbox_material".to_string(), walkbox_material.clone_untyped());
}

fn setup_level(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        LdtkWorld,
        LdtkWorldBundle {
            ldtk_handle: asset_server.load("kittytown.ldtk"),
            transform: Transform::from_scale(Vec3::splat(PIXEL_SCALE)),
            ..Default::default()
        },
    ));
}

fn setup_camera(
    mut commands: Commands,
) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scale = 1.0/3.0;
    commands.spawn((
        camera_bundle,
        SubTransform{ translation: Vec3::new(0.0, 0.0, 999.0) },
        // ^^ hack: I looked up the Z coord on new_2D and fudged it so we won't accidentally round it to 1000.
    ));
}

fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    debug_assets: Res<DebugAssets>,
) {
    let run: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRun.aseprite");
    let idle: Handle<CharAnimation> = asset_server.load("sprites/sPlayer.aseprite");
    let hurt: Handle<CharAnimation> = asset_server.load("sprites/sPlayerHurt.aseprite");
    let roll: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRoll.aseprite");

    let initial_animation = idle.clone();

    // Okay, I'm going to be lazy here: hashmap w/ string literals. SORRY. I'll
    // come back to it later.
    let mut animations: AnimationsMapInner = HashMap::new();
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
            sub_transform: SubTransform{ translation: Vec3::new(0.0, 0.0, 3.0) },
            speed: Speed(Speed::RUN),
            // --- New animation system
            char_animation_state: CharAnimationState::new(initial_animation, Dir::E),
            motion: Motion::new(Vec2::ZERO),
            animations_map: AnimationsMap(animations),
        },
        // Initial gameplay state
        // PlayerFree::new(),
        PlayerRoll::new(0.785),
    )).with_children(|player| {
        let (Some(mesh_untyped), Some(material_untyped)) = (debug_assets.get("walkbox_mesh"), debug_assets.get("walkbox_material"))
        else {
            warn!("Hey!! I couldn't get ahold of the collider debug assets for some reason!!");
            return;
        };
        let material = material_untyped.clone().typed::<ColorMaterial>();
        let mesh = Mesh2dHandle(mesh_untyped.clone().typed::<Mesh>());

        player.spawn(WalkboxDebugBundle {
            mesh_bundle: MaterialMesh2dBundle {
                mesh,
                material,
                visibility: Visibility { is_visible: false },
                ..default()
            },
            ..default()
        });
    });
}

fn debug_walkboxes_system(
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

#[derive(Bundle)]
struct PlayerBundle {
    identity: Player,
    sprite_sheet: SpriteSheetBundle,

    sub_transform: SubTransform,
    speed: Speed,

    char_animation_state: CharAnimationState,
    motion: Motion,
    animations_map: AnimationsMap,
}

// Structs and crap!

type AnimationsMapInner = HashMap<&'static str, Handle<CharAnimation>>;

#[derive(Component)]
pub struct AnimationsMap(AnimationsMapInner);
impl Deref for AnimationsMap {
    type Target = AnimationsMapInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
    pub distance_remaining: f32,
    pub just_started: bool,
    /// The "player input" equivalent to use when planning motion; a length-1.0
    /// vector locked to the direction we were facing when starting our roll.
    pub roll_input: Vec2,
    pub transition: PlayerRollTransition,
}
#[derive(Clone)]
pub enum PlayerRollTransition {
    None,
    Bonk,
    Free,
}
impl Default for PlayerRollTransition {
    fn default() -> Self {
        Self::None
    }
}
impl PlayerRoll {
    const DISTANCE: f32 = 52.0;

    pub fn new(direction: f32) -> Self {
        PlayerRoll {
            distance_remaining: Self::DISTANCE,
            just_started: true,
            roll_input: Vec2::from_angle(direction),
            transition: PlayerRollTransition::None,
        }
    }
}
/// Player state: bonkin'
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PlayerBonk;


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
    /// The planned motion for the current frame, which a variety of things might be interested in.
    pub planned: Vec2,
    /// The direction the entity is currently facing, in radians. Tracked
    /// separately because it persists even when no motion is planned.
    pub direction: f32,
    pub remainder: Vec2,
    pub result: Option<MotionResult>,
}
impl Motion {
    pub fn new(motion: Vec2) -> Self {
        let mut thing = Self {
            planned: Vec2::ZERO,
            direction: 0.0, // facing east on the unit circle
            remainder: Vec2::ZERO,
            result: None,
        };
        thing.update_plan(motion);
        thing
    }

    pub fn update_plan(&mut self, motion: Vec2) {
        if motion == Vec2::ZERO {
            self.remainder = Vec2::ZERO;
        } else {
            self.direction = Vec2::X.angle_between(motion);
        }
        self.planned = motion;
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

/// BBox defining the space an entity takes up on the ground.
#[derive(Component, Inspectable)]
pub struct Walkbox(Rect);

/// BBox defining the space where an entity can be hit by attacks.
#[derive(Component, Inspectable)]
pub struct Hitbox(Rect);
// ...and then eventually I'll want Swingbox for attacks, but, tbh I have no
// idea how to best handle that yet. Is that even a component? Or is it a larger
// data structure associated with an animation or something?

pub fn centered_rect(width: f32, height: f32) -> Rect {
    let min = Vec2::new(-width/2., -height/2.);
    let max = Vec2::new(width/2., height/2.);
    Rect {
        min,
        max,
    }
}

pub fn bottom_centered_rect(width: f32, height: f32) -> Rect {
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

#[derive(Bundle, Default)]
struct WalkboxDebugBundle {
    mesh_bundle: MaterialMesh2dBundle<ColorMaterial>,
    marker: WalkboxDebug,
}
/// Marker component for walkbox debug mesh
#[derive(Component, Default)]
struct WalkboxDebug;
#[derive(Resource, Inspectable, Default)]
struct DebugColliders {
    show_walkboxes: bool,
}

#[derive(Resource, Deref, DerefMut, Default)]
struct DebugAssets(HashMap<String, HandleUntyped>);

/// Collidable solid component... but you also need a position Vec3 and a size Vec2 from somewhere.
#[derive(Component)]
pub struct Solid;

/// Wall bundle for tilemap walls
#[derive(Bundle)]
struct Wall {
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

// Can't spawn child entities in bundle_int_cell, alas, so need an after-the-fact setup
fn spawn_wall_tile_collider_debugs(
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
