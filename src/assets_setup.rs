use crate::char_animation::CharAnimation;
use bevy::prelude::*;
use std::collections::HashMap;

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

#[derive(Resource, Deref, DerefMut, Default)]
pub struct AnimationsMap(HashMap<Ases, Handle<CharAnimation>>);

/// Sets up a shared hashmap resource of loaded animated sprite assets.
pub fn load_sprite_assets(asset_server: Res<AssetServer>, mut animations: ResMut<AnimationsMap>) {
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
