//! Systems for camera work. I don't especially expect this to be a well-separated
//! module, because camera logic is so tied to specific gameplay. So, it's okay to
//! just use shit from main.

use crate::{
    phys_space::{PhysOffset, PhysTransform},
    Player,
};
use bevy::prelude::*;

pub fn setup_camera(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scale = 1.0 / 5.0;
    commands.spawn((
        camera_bundle,
        PhysOffset(Vec2::ZERO),
        PhysTransform {
            translation: Vec3::new(0.0, 0.0, 999.0),
        },
        // ^^ hack: I looked up the Z coord on new_2D and fudged it so we won't accidentally round it to 1000.
    ));
}

pub fn camera_lerp_system(
    time: Res<Time>,
    // time: Res<StaticTime>,
    // time: Res<SmoothedTime>,
    mut params: ParamSet<(
        Query<&PhysTransform, With<Player>>,
        Query<&mut PhysTransform, With<Camera>>,
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

pub fn camera_locked_system(
    mut params: ParamSet<(
        Query<&PhysTransform, With<Player>>,
        Query<&mut PhysTransform, With<Camera>>,
    )>,
) {
    let player_pos = params.p0().single().translation;
    let mut camera_q = params.p1();
    let mut camera_tf = camera_q.single_mut();
    camera_tf.translation.x = player_pos.x;
    camera_tf.translation.y = player_pos.y;
}
