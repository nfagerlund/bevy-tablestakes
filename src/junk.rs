use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
// use bevy_ecs_ldtk::prelude::*;
use bevy_ecs_tilemap::prelude::*;

pub fn debug_z_system(
    // mut local_timer: Local<Timer>,
    player_query: Query<&Transform, With<crate::Player>>,
    world_query: Query<&Transform, With<crate::LdtkWorld>>,
    level_query: Query<(Entity, &Transform, &Map)>,
) {
    let player_transform = player_query.get_single().unwrap();
    let world_transform = world_query.get_single().unwrap();
    info!("Player at: {}\n World at: {}\n", player_transform.translation, world_transform.translation);
    for (e_id, transform, map) in level_query.iter() {
        info!("  Level {:?} (map id {}) at {}\n", e_id, map.id, transform.translation);
    }
}

pub fn setup_fps_debug(
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

// again borrowed from bevymark example
pub fn update_fps_debug_system(
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

// structs and crap!

/// Marker component for FPS counter
#[derive(Component)]
pub struct FPSCounter;

