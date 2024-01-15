use bevy::prelude::*;

#[derive(Resource, Default, Reflect, PartialEq, Eq)]
pub struct DebugSettings {
    pub debug_walkboxes: bool,
    pub debug_origins: bool,
    pub debug_hitboxes: bool,
    pub motion_kind: MotionKind,
    pub camera_kind: CameraKind,
}

#[derive(Resource, Reflect, PartialEq)]
pub struct NumbersSettings {
    pub launch_gravity: f32,
    pub player_bonk_z_velocity: f32,
}

impl Default for NumbersSettings {
    fn default() -> Self {
        Self {
            launch_gravity: crate::behaviors::LAUNCH_GRAVITY,
            player_bonk_z_velocity: crate::PlayerState::BONK_Z_VELOCITY,
        }
    }
}

#[derive(Resource, Reflect, Default, PartialEq, Eq)]
pub enum MotionKind {
    NoCollision,
    Faceplant,
    #[default]
    RayTest,
    WholePixel,
}

#[derive(Resource, Reflect, Default, PartialEq, Eq)]
pub enum CameraKind {
    #[default]
    Locked,
    Lerp,
}

pub fn motion_is(kind: MotionKind) -> impl Fn(Res<DebugSettings>) -> bool {
    move |debugs: Res<DebugSettings>| debugs.motion_kind == kind
}
pub fn camera_is(kind: CameraKind) -> impl Fn(Res<DebugSettings>) -> bool {
    move |debugs: Res<DebugSettings>| debugs.camera_kind == kind
}
