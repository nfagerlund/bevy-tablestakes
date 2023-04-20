use crate::char_animation::*;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::sprite::ExtractedSprites;
use bevy_inspector_egui::Inspectable;

const DEPTH_DUDES: f32 = 4.0;
const DEPTH_SHADOWS: f32 = DEPTH_DUDES - 0.1;

/// Some spatial details about an entity.
#[derive(Component, Inspectable)]
pub struct TopDownMatter {
    /// For now, depth tracks the absolute, global draw-depth coordinate for
    /// anything it's placed on. I'm not doing any Y-sorting yet. An extract
    /// system uses this value to overwrite the entity's Z coordinate in the
    /// render world.
    pub depth: f32,
    /// If false, the entity can rise into the air. If true, it remains fixed on
    /// the ground (like a shadow), and manipulating its main world Z coordinate
    /// (including in the transform_propagate_system) does nothing.
    pub ignore_height: bool,
}

impl TopDownMatter {
    pub fn character() -> Self {
        Self {
            depth: DEPTH_DUDES,
            ignore_height: false,
        }
    }
    pub fn shadow() -> Self {
        Self {
            depth: DEPTH_SHADOWS,
            ignore_height: true,
        }
    }
}

impl Default for TopDownMatter {
    fn default() -> Self {
        Self::character()
    }
}

/// Marker struct for things that cast a simple shadow on the ground.
#[derive(Component)]
pub struct HasShadow;

/// Marker struct for the shadow itself.
#[derive(Component)]
pub struct ShadowSprite;

/// Bundle that actually implements a simple shadow child entity.
#[derive(Bundle)]
pub struct ShadowSpriteBundle {
    identity: ShadowSprite,
    sprite_sheet: SpriteSheetBundle,
    char_animation_state: CharAnimationState,
    topdown_matter: TopDownMatter,
}

impl ShadowSpriteBundle {
    fn new(handle: Handle<CharAnimation>) -> Self {
        Self {
            identity: ShadowSprite,
            sprite_sheet: SpriteSheetBundle {
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, -0.1)),
                ..default()
            },
            char_animation_state: CharAnimationState::new(
                handle,
                VariantName::Neutral,
                Playback::Loop,
            ),
            topdown_matter: TopDownMatter::shadow(),
        }
    }
}

/// Attach shadow sprite child entities to anything new that HasShadow.
pub fn shadow_stitcher_system(
    mut shadow_handle: Local<Option<Handle<CharAnimation>>>,
    asset_server: Res<AssetServer>,
    new_shadow_q: Query<Entity, Added<HasShadow>>,
    mut commands: Commands,
) {
    // Will need to populate shadow handle on first system run:
    if shadow_handle.is_none() {
        info!("populating shadow handle in a local");
        *shadow_handle = Some(asset_server.load("sprites/sShadow.aseprite"));
    }
    // but, will still need to perform normal business that first run! At this
    // point, .unwrap() is fine because we made sure there's something there,
    // and if that goes wonky I want early warning anyway.
    for shadow_owner in new_shadow_q.iter() {
        info!("stitching a shadow to {:?}", &shadow_owner);
        commands.entity(shadow_owner).with_children(|parent| {
            parent.spawn(ShadowSpriteBundle::new(
                shadow_handle.as_ref().unwrap().clone(),
            ));
        });
    }
}

/// Extract system to translate the in-game x/y/z-height coordinates to the
/// draw-relevant x/y/z-depth coordiantes. Offsets Y by Z, and will eventually
/// do Y-sorting for drawing things in front of each other.
pub fn extract_and_flatten_space_system(
    has_z_query: Extract<Query<&TopDownMatter>>,
    mut extracted_sprites: ResMut<ExtractedSprites>,
) {
    // Well it's deeply unfortunate, but because the extract sprites system
    // crams everything into a Vec stored as a resource, we've got to iterate
    // over that and correlate it with our query.
    for ex_sprite in extracted_sprites.sprites.iter_mut() {
        if let Ok(matter) = has_z_query.get(ex_sprite.entity) {
            let transform = ex_sprite.transform.translation_mut();
            if !matter.ignore_height {
                transform.y += transform.z;
            }
            transform.z = matter.depth;
        }
    }
    // Also!! Counterpoint! It makes somewhat more sense to do this in the
    // actual extract run for sprites so you don't have to double-handle these
    // ExtractedSprite structs. And extract_sprites is short, and so is the
    // Plugin::build() impl for SpritePlugin. I could just make my own custom
    // SpritePlugin and remove the stock one from DefaultPlugins.
}
