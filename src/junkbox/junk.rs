use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_ecs_ldtk::prelude::*;

use crate::{
    collision::{AbsBBox, Walkbox},
    movement::Motion,
    phys_space::PhysTransform,
};
// use bevy_ecs_tilemap::prelude::*;

pub fn _overlap_chaperone(movers_q: Query<(Entity, &Walkbox, &PhysTransform), With<Motion>>) {
    for [(a_ent, a_walkbox, a_transform), (b_ent, b_walkbox, b_transform)] in
        movers_q.iter_combinations()
    {
        let a_bbox = AbsBBox::from_rect(a_walkbox.0, a_transform.translation.truncate());
        let b_bbox = AbsBBox::from_rect(b_walkbox.0, b_transform.translation.truncate());
        if a_bbox.collide(b_bbox) {
            info!(
                "Hanky-panky detected between {:?} and {:?} \n ({:.8?}) \n ({:.8?})",
                a_ent, b_ent, a_bbox, b_bbox
            );
        }
    }
}

pub fn _debug_z_system(
    // mut local_timer: Local<Timer>,
    player_query: Query<&Transform, With<crate::Player>>,
    world_query: Query<&Transform, With<crate::LdtkWorld>>,
    level_query: Query<(Entity, &Transform, &bevy_ecs_tilemap::map::TilemapId)>,
) {
    let player_transform = player_query.get_single().unwrap();
    let world_transform = world_query.get_single().unwrap();
    info!(
        "Player at: {}\n World at: {}\n",
        player_transform.translation, world_transform.translation
    );
    for (e_id, transform, map) in level_query.iter() {
        info!(
            "  Level {:?} (map id {:?}) at {}\n",
            e_id, map.0, transform.translation
        );
    }
}

pub fn _tile_info_barfing_system(
    keys: Res<ButtonInput<KeyCode>>,
    tile_query: Query<(&IntGridCell, &GridCoords, &Transform)>,
    level_query: Query<(&LevelIid, &Transform)>,
) {
    if keys.just_pressed(KeyCode::KeyB) {
        for (gridcell, _coords, transform) in tile_query.iter() {
            info!("{:?} at {:?}", gridcell, transform);
        }
        for (level, transform) in level_query.iter() {
            info!("level {:?} at {:?}", level, transform);
        }
    }
}

pub fn _setup_fps_debug(mut commands: Commands, asset_server: Res<AssetServer>) {
    let style = TextStyle {
        font: asset_server.load("fonts/m5x7.ttf"),
        font_size: 32.0,
        color: Color::srgb(0.0, 1.0, 0.0),
    };
    // borrowing this from the bevymark example
    commands.spawn((
        FPSCounter,
        TextBundle {
            text: Text {
                sections: vec![
                    TextSection {
                        value: "FPS: ".to_string(),
                        style: style.clone(),
                    },
                    TextSection {
                        value: "".to_string(),
                        style,
                    },
                ],
                ..Default::default() // alignment
            },
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                ..Default::default() // boy, LOTS of these
            },
            ..Default::default()
        },
    ));
}

// again borrowed from bevymark example
pub fn _update_fps_debug_system(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FPSCounter>>,
) {
    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
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
