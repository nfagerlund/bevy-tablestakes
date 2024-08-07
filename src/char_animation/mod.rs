use bevy::prelude::*;

use crate::compass::Dir;
use crate::render::TopDownMatter;
use crate::Motion;

// Breaking stuff up for organization, but functionally this is just one thing from the outside.
mod assets;
mod systems;
mod types;
pub use self::systems::*;
pub use self::types::*;

/// GOOFUS SYSTEM: Follow the birdie
fn charanm_test_set_motion_system(
    mut query: Query<&mut Motion, With<Goofus>>,
    inputs: Res<crate::input::CurrentInputs>,
) {
    for mut motion in query.iter_mut() {
        motion.face(inputs.movement * -1.0);
    }
}

/// GOOFUS SYSTEM: Spawn
fn charanm_test_setup_system(mut commands: Commands, asset_server: Res<AssetServer>) {
    let anim_handle: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRun.aseprite");
    commands.spawn((
        Goofus,
        Name::new("Goofus"),
        TopDownMatter::character(),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(30.0, 60.0, 0.0)),
            ..default()
        },
        TextureAtlas::default(),
        crate::render::HasShadow,
        CharAnimationState::new(anim_handle, Dir::W, Playback::Loop),
        Motion::new(Vec2::ZERO),
    ));
}

/// GOOFUS: an animation test entity who does the opposite of player inputs.
#[derive(Component)]
struct Goofus;

/// GOOFUS PLUGIN: animation test
pub struct TestCharAnimationPlugin;

impl Plugin for TestCharAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, charanm_test_setup_system)
            .add_systems(Update, charanm_test_set_motion_system);
    }
}
