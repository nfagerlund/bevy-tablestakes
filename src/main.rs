use bevy::ecs::component;
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
use std::collections::HashMap;
use std::ops::Deref;
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
        .register_inspectable::<Speed>()
        .register_inspectable::<Walkbox>()
        .register_inspectable::<Hitbox>()

        // LDTK STUFF
        .add_startup_system(setup_level)
        .insert_resource(LevelSelection::Index(1))
        .register_ldtk_int_cell_for_layer::<Wall>("StructureKind", 1)

        // CAMERA
        .add_startup_system(setup_camera)

        // INPUT STUFF
        .add_system(connect_gamepads_system)
        .insert_resource(CurrentInputs::default())
        .add_system_to_stage(CoreStage::PreUpdate, accept_input_system)

        // PLAYER STUFF
        .add_startup_system(setup_player)
        .add_system(move_player_system)
        // .add_system(charanm_test_animate_system) // in CharAnimationPlugin or somethin'
        .add_system(dumb_move_camera_system.after(move_player_system))
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

fn move_and_collide(
    origin: Vec2,
    movement_intent: Vec2,
    walkbox: Rect,
    solids: &[AbsBBox],
) -> Movement {
    let mut location = origin;
    let mut collided = false;

    let mut move_x = movement_intent.x;
    let sign_x = move_x.signum();
    while move_x != 0. {
        let next_location = location + Vec2::new(sign_x, 0.0);
        let next_box = AbsBBox::from_rect(walkbox, next_location);
        if let None = solids.iter().find(|s| s.collide(next_box)) {
            location.x += sign_x;
            move_x -= sign_x;
        } else {
            // Hit a wall
            collided = true;
            break;
        }
    }
    let mut move_y = movement_intent.y;
    let sign_y = move_y.signum();
    while move_y != 0. {
        let next_origin = location + Vec2::new(0.0, sign_y);
        let next_box = AbsBBox::from_rect(walkbox, next_origin);
        if let None = solids.iter().find(|s| s.collide(next_box)) {
            location.y += sign_y;
            move_y -= sign_y;
        } else {
            // Hit a wall
            collided = true;
            break;
        }
    }

    Movement {
        collided,
        new_location: location,
    }
}

pub struct Movement {
    pub collided: bool,
    pub new_location: Vec2,
}

fn move_player_system(
    inputs: Res<CurrentInputs>,
    time: Res<Time>,
    mut player_q: Query<(&mut SubTransform, &mut Motion, &mut MoveRemainder, &Speed, &Walkbox, &AnimationsMap, &mut CharAnimationState), (With<Player>, With<PlayerFree>)>,
    solids_q: Query<(&Transform, &Walkbox), With<Solid>>,
) {
    // Take the chance to exit early if there's no suitable player:
    let Ok((mut player_tf, mut motion, mut move_remainder, speed, walkbox, animations_map, mut animation_state)) = player_q.get_single_mut()
    else { // rust 1.65 baby
        println!("Zero players found! Probably missing walkbox or something.");
        return;
    };

    let delta = time.delta_seconds();
    let move_input = inputs.movement;
    let raw_movement_intent = move_input * speed.0 * delta;

    // Publish movement intent for anyone who needs it later (this is where
    // sprite direction gets derived from)
    motion.0 = raw_movement_intent;

    // If we're not moving, stop running and bail. Right now I'm not doing
    // change detection, so I can just do an unconditional hard assign w/ those
    // handles...
    if raw_movement_intent.length() == 0.0 {
        let idle = animations_map.get("idle").unwrap().clone();
        animation_state.change_animation(idle);
        return;
    }

    // OK, we're running
    let run = animations_map.get("run").unwrap().clone();
    animation_state.change_animation(run);

    // Determine the actual pixel distance we're going to try to move, and stash the remainder
    move_remainder.0 += raw_movement_intent;
    let move_pixels = move_remainder.0.round();
    move_remainder.0 -= move_pixels;

    let solids: Vec<AbsBBox> = solids_q.iter().map(|(transform, walkbox)| {
        let origin = transform.translation.truncate();
        AbsBBox::from_rect(walkbox.0, origin)
    }).collect();

    // See where we were actually able to move to, and what happened:
    let movement = move_and_collide(
        player_tf.translation.truncate(),
        move_pixels,
        walkbox.0,
        &solids,
    );

    // Commit it
    player_tf.translation.x = movement.new_location.x;
    player_tf.translation.y = movement.new_location.y;
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
    commands
        .spawn_bundle(SpriteSheetBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0))
                .with_scale(Vec3::splat(PIXEL_SCALE)),
            ..Default::default()
        })

        // --- New animation system
        .insert(CharAnimationState::new(initial_animation, Dir::E))
        .insert(Motion(Vec2::ZERO))
        .insert(AnimationsMap(animations))

        // Initial values
        .insert(SubTransform{ translation: Vec3::new(0.0, 0.0, 3.0) })
        .insert(MoveRemainder(Vec2::ZERO))
        .insert(Speed(Speed::RUN))
        .insert(PlayerFree)

        // Remember who u are
        .insert(Player);
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
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct PlayerFree;
// ^^ That SparseSet storage might be useful or might not (I haven't profiled
// it), but this IS the specific use case it is suggested for (marker components
// that don't contain much data, don't get iterated over en masse on the
// regular, and get added and removed extremely frequently.)

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
    const RUN: f32 = 180.0;
}

/// The intended motion for the current frame, which a variety of things might be interested in.
#[derive(Component)]
pub struct Motion(Vec2);

#[derive(Component)]
pub struct MoveRemainder(Vec2);

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


/// Collidable solid component... but you also need a position Vec3 and a size Vec2 from somewhere.
#[derive(Component)]
pub struct Solid;

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
