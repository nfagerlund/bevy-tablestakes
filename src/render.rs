use crate::char_animation::*;
use crate::collision::AbsBBox;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::sprite::ExtractedSprites;

const DEPTH_DUDES_MIN: f32 = 4.0;
const DEPTH_DUDES_MAX: f32 = 50.0;
const DEPTH_DUDES_RANGE: f32 = DEPTH_DUDES_MAX - DEPTH_DUDES_MIN;
const DEPTH_SHADOWS: f32 = DEPTH_DUDES_MIN - 0.1;
const VIEW_SLOP: f32 = 64.0;

fn lerp_dudes_z(t: f32) -> f32 {
    DEPTH_DUDES_MIN + DEPTH_DUDES_RANGE * t
}

/// Some spatial details about an entity.
#[derive(Component, Reflect)]
pub struct TopDownMatter {
    /// How the global draw depth should be determined. Depth is calculated
    /// differently for different kinds of stuff. A render
    /// system uses this value to overwrite the entity's Z (depth) coordinate in the
    /// render world.
    pub depth_class: TopDownDepthClass,
    /// If false, the entity can rise into the air. If true, it remains fixed on
    /// the ground (like a shadow), and manipulating its main world Z coordinate
    /// (including in the transform_propagate_system) does nothing.
    pub ignore_height: bool,
}

#[derive(Reflect)]
pub enum TopDownDepthClass {
    Character,
    Shadow,
}

impl TopDownMatter {
    pub fn character() -> Self {
        Self {
            depth_class: TopDownDepthClass::Character,
            ignore_height: false,
        }
    }
    pub fn shadow() -> Self {
        Self {
            depth_class: TopDownDepthClass::Shadow,
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
    // point, we know there's something in there.
    let Some(sh) = shadow_handle.as_ref() else {
        warn!("shadow handle missing, this should be impossible??");
        return;
    };
    for shadow_owner in new_shadow_q.iter() {
        info!("stitching a shadow to {:?}", &shadow_owner);
        commands.entity(shadow_owner).with_children(|parent| {
            parent.spawn(ShadowSpriteBundle::new(sh.clone()));
        });
    }
}

/// Extract system to translate the in-game x/y/z-height coordinates to the
/// draw-relevant x/y/z-depth coordiantes. Offsets Y by Z, and does Y-sorting
/// for drawing things in front of each other.
/// TODO: you're not supposed to do very much in `ExtractSchedule`, so maybe
/// split this into an extract matter/viewport system and a flatten space system.
/// Counterpoint: this is small.
pub fn extract_and_flatten_space_system(
    has_z_query: Extract<Query<(Entity, &TopDownMatter)>>,
    camera_query: Extract<Query<(&OrthographicProjection, &GlobalTransform), With<Camera2d>>>,
    mut extracted_sprites: ResMut<ExtractedSprites>,
) {
    // ok, my theory goes like this:
    // - Figure out the range of visible global Y values
    // - Decide ahead of time the range of usable Z values for characters
    // - If a sprite is maybe visible, place it in the Z band proportional to its place
    //   in the Y band.
    // So, first, sort out the viewport.
    let y_frac = {
        // I'm gonna be dumb and assume there's one camera, for now. call me once there's not.
        let Ok((projection, cam_transform)) = camera_query.get_single() else {
            warn!("camera_qurey.get_single exploded in extract_and_flatten_space");
            return;
        };
        let viewport = AbsBBox::from_rect(projection.area, cam_transform.translation().truncate());
        let min_y = viewport.min.y - VIEW_SLOP;
        let max_y = viewport.max.y + VIEW_SLOP;
        let y_size = max_y - min_y;

        move |y: f32| (max_y - y) / y_size
    };

    // NICE, ExtractedSprites uses EntityHashMap now, so I only
    // need to iterate over sprites that are topdown-matter.
    for (entity, matter) in has_z_query.iter() {
        if let Some(ex_sprite) = extracted_sprites.sprites.get_mut(&entity) {
            let mut translation = ex_sprite.transform.translation();
            let depth = match matter.depth_class {
                TopDownDepthClass::Character => {
                    // OK, I think we can just yolo this without bounds-checking,
                    // bc if you're outside the viewport it just.......... shouldn't matter
                    lerp_dudes_z(y_frac(translation.y))
                },
                TopDownDepthClass::Shadow => DEPTH_SHADOWS,
            };
            if !matter.ignore_height {
                translation.y += translation.z;
            }
            translation.z = depth;
            ex_sprite.transform = Transform::from_translation(translation).into();
        }
    }
}
