use bevy::asset::{
    AssetServer, AssetLoader, BoxedFuture, Handle, LoadContext, LoadedAsset,
};
use bevy::math::prelude::*;
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::{
    render_resource::{Extent3d, TextureDimension, TextureFormat},
    texture::{Image, TextureFormatPixelInfo}
};
use bevy::sprite::{Rect, TextureAtlas};
use bevy::utils::Duration;
use std::collections::HashMap;
use asefile::AsepriteFile;
use image::RgbaImage;

use crate::compass::{self, Dir};

#[derive(Debug, TypeUuid)]
#[uuid = "585e2e41-4a97-42ef-a13e-55761c854bb4"]
pub struct CharAnimation {
    pub variants: VariantsMap,
    pub texture_atlas: Handle<TextureAtlas>,
}

#[derive(Debug)]
pub struct CharAnimationVariant {
    pub name: VariantName,
    pub frames: Vec<CharAnimationFrame>,
}

// I've changed my mind about tags: I'm gonna start out with a rigid enum,
// enforce tag names at load time, and expand behavior as needed later. Duping
// the Dir type for easier refactors later??
pub type VariantName = compass::Dir;
type VariantsMap = HashMap<VariantName, CharAnimationVariant>;

/// Data for an individual animation frame. This struct contains coordinates for
/// some points and rectangles; in all cases, these are relative to the top left
/// corner of the sprite.
#[derive(Debug)]
pub struct CharAnimationFrame {
    /// Index into the `TextureAtlas`.
    pub index: usize,
    /// Frame duration.
    pub duration: Duration,
    /// Origin/anchor/pivot point of the sprite. Stored on the frame for
    /// convenience, but shouldn't vary.
    pub origin: Vec2,
    /// Bbox for the projected foot position on the ground.
    pub walkbox: Option<Rect>,
    // todo: hitbox, hurtbox,
}

pub struct CharAnimationPlugin;

impl Plugin for CharAnimationPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_asset::<CharAnimation>()
            .init_asset_loader::<CharAnimationLoader>();
    }
}

#[derive(Default)]
pub struct CharAnimationLoader;

impl AssetLoader for CharAnimationLoader {
    fn extensions(&self) -> &[&str] {
        &["aseprite", "ase"]
    }

    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            load_aseprite(bytes, load_context)?;
            Ok(())
        })
    }
}

/// Loads an aseprite file and uses it to construct a sprite sheet `#texture`, a
/// `#texture_atlas` that indexes into that sprite sheet, and a top-level
/// `CharAnimation`. The individual `CharAnimationFrames` in the
/// `CharAnimationVariants` contain indexes into the `TextureAtlas`.
/// Assumptions:
/// - File only uses AnimationDirection::Forward.
/// - Tag names are unique in the file. (Aseprite doesn't guarantee this.)
/// - Named tags cover all of the needed animation frames.
/// - Walkbox layer: "walkbox"
/// - Origin layer: "origin"
/// - Layers for drawn-on metadata coordinates should be marked as invisible in
///   the saved file.
fn load_aseprite(bytes: &[u8], load_context: &mut LoadContext) -> anyhow::Result<()> {
    let ase = AsepriteFile::read(bytes)?;
    let width = ase.width();
    let height = ase.height();
    let num_frames = ase.num_frames();

    // Build the texture atlas, ensuring that its sub-texture indices match the
    // original Aseprite file's frame indices.
    let frame_images: Vec<Image> = (0..num_frames)
        .map(|i| remux_image(ase.frame(i).image()))
        .collect();
    // Atlas will be a 1D horizontal strip w/ 1px padding between frames.
    let atlas_height = height as u32;
    let atlas_width = width as u32 * num_frames + num_frames - 1;
    let mut atlas_texture = Image::new_fill(
        Extent3d { width: atlas_width, height: atlas_height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 0], // clear
        TextureFormat::Rgba8UnormSrgb, // Could frame_images[0].format(), but hardcode for now.
    );
    // copy time
    let mut cur_x = 0_usize;
    for img in frame_images.iter() {
        copy_texture_to_atlas(
            &mut atlas_texture,
            img,
            width,
            height,
            cur_x,
            0,
        );
        cur_x += width + 1;
    }
    // stow texture and get a handle
    let texture_handle = load_context.set_labeled_asset(
        "texture",
        LoadedAsset::new(atlas_texture),
    );
    // atlas time!!!
    // N.b.: from_grid_with_padding adds grid cells in left-to-right,
    // top-to-bottom order, and we rely on this to make the frame indices match.
    let atlas = TextureAtlas::from_grid_with_padding(
        texture_handle,
        Vec2::new(width as f32, height as f32),
        num_frames as usize,
        1,
        Vec2::new(1.0, 0.0),
        Vec2::ZERO,
    );
    let atlas_handle = load_context.set_labeled_asset(
        "texture_atlas",
        LoadedAsset::new(atlas),
    );

    // Since our final frame indices are reliable, processing tags is easy.
    let mut variants: VariantsMap = HashMap::new();

    for tag in (0..ase.num_tags()).map(|i| ase.tag(i)) {
        let name: VariantName = tag.name().try_into()?; // Just propagate error, don't continue load.
        let frame_range = tag.from_frame()..=tag.to_frame(); // inclusive
        let frames: Vec<CharAnimationFrame> = frame_range.map(|i| {
            let frame = ase.frame(i);
            let index = i as usize;
            let duration_ms = frame.duration() as u64;
            let duration = Duration::from_millis(duration_ms);

            let walkbox = rect_from_cel(&ase, "walkbox", i);

            // Wasteful, bc we could exit early on first non-clear px, but meh.
            let origin = match rect_from_cel(&ase, "origin", i) {
                Some(origin_rect) => origin_rect.min,
                None => Vec2::ZERO, // Origin's non-optional.
            };

            CharAnimationFrame {
                index,
                duration,
                origin,
                walkbox,
            }
        }).collect();

        let variant = CharAnimationVariant { name, frames };
        variants.insert(name, variant);
    }

    // The whole enchilada:
    let animation = CharAnimation { variants, texture_atlas: atlas_handle };

    // And, cut!
    load_context.set_default_asset(LoadedAsset::new(animation));
    Ok(())
}

/// Convert the image buffer returned by `asefile::Frame.image()` into a
/// `bevy::render::texture::Image`. Consumes the argument and re-uses the
/// internal container.
fn remux_image(img: RgbaImage) -> Image {
    let size = Extent3d {
        width: img.width(),
        height: img.height(),
        depth_or_array_layers: 1
    };
    Image::new(
        size,
        TextureDimension::D2,
        img.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
    )
}

/// Get the bounding Rect for a cel's non-transparent pixels.
fn rect_from_cel(ase: &AsepriteFile, layer_name: &str, frame_index: u32) -> Option<Rect> {
    ase.layer_by_name(layer_name).map(|layer| {
        let cel_img = layer.frame(frame_index).image();
        get_rect_lmao(&cel_img)
    }).flatten()
}

/// Get the bounding Rect for the non-transparent pixels in an RgbaImage.
fn get_rect_lmao(img: &RgbaImage) -> Option<Rect> {
    let mut x_min: u32 = u32::MAX;
    let mut x_max: u32 = 0;
    let mut y_min: u32 = u32::MAX;
    let mut y_max: u32 = 0;
    let mut present = false;
    for (x, y, val) in img.enumerate_pixels() {
        if non_empty(val) {
            present = true;
            if x < x_min { x_min = x; }
            if x > x_max { x_max = x; }
            if y < y_min { y_min = y; }
            if y > y_max { y_max = y; }
        }
    }
    if present {
        Some(Rect {
            min: Vec2::new(x_min as f32, y_min as f32),
            max: Vec2::new(x_max as f32, y_max as f32),
        })
    } else {
        None
    }
}

fn alpha(pixel: &image::Rgba<u8>) -> u8 {
    pixel.0[3]
}

fn non_empty(pixel: &image::Rgba<u8>) -> bool {
    alpha(pixel) != 0
}

// Variation on a TextureAtlasBuilder fn (which I can't use directly bc it
// relies on runtime asset collections):
// https://github.com/bevyengine/bevy/blob/c27cc59e0/crates/bevy_sprite/src/texture_atlas_builder.rs#L95
fn copy_texture_to_atlas(
    atlas_texture: &mut Image,
    texture: &Image,
    rect_width: usize,
    rect_height: usize,
    rect_x: usize,
    rect_y: usize,
) {
    let atlas_width = atlas_texture.texture_descriptor.size.width as usize;
    let format_size = atlas_texture.texture_descriptor.format.pixel_size();

    for (texture_y, bound_y) in (rect_y..rect_y + rect_height).enumerate() {
        let begin = (bound_y * atlas_width + rect_x) * format_size;
        let end = begin + rect_width * format_size;
        let texture_begin = texture_y * rect_width * format_size;
        let texture_end = texture_begin + rect_width * format_size;
        atlas_texture.data[begin..end]
            .copy_from_slice(&texture.data[texture_begin..texture_end]);
    }
}

// Right, so, how do we animate these dudes? (and thus, what do we need in our state component/bundle?)
// Starting in media res:
// - Each step, tick down a TIMER (non-repeating) for the current frame.
// - When the timer runs out, switch your FRAME INDEX to the next frame (or, WRAP AROUND if you're configured to loop).

#[derive(Component, Debug)]
pub struct TempCharAnimationState {
    animation: Handle<CharAnimation>,
    variant: Option<VariantName>, // hate the string lookup here btw, need something better.
    // guess I could do interning and import IndexMap maybe.
    frame: usize,
    frame_timer: Option<Timer>,
    looping: bool,
}

impl TempCharAnimationState {
    fn new(animation: Handle<CharAnimation>, variant: VariantName, looping: bool) -> Self {
        TempCharAnimationState {
            animation,
            variant: Some(variant),
            // in the future I might end up wanting to blend between animations
            // at a particular frame. Doesn't matter yet tho.
            frame: 0,
            frame_timer: None,
            looping,
        }
    }

    fn change_variant(&mut self, variant: VariantName) {
        self.variant = Some(variant);
        self.frame = 0;
        self.frame_timer = None;
    }
}

fn charanm_test_directions_system(
    mut query: Query<&mut TempCharAnimationState>,
    keys: Res<Input<KeyCode>>,
) {
    for mut state in query.iter_mut() {
        if keys.just_pressed(KeyCode::Right) {
            state.change_variant(Dir::E);
        } else if keys.just_pressed(KeyCode::Up) {
            state.change_variant(Dir::N);
        } else if keys.just_pressed(KeyCode::Left) {
            state.change_variant(Dir::W);
        } else if keys.just_pressed(KeyCode::Down) {
            state.change_variant(Dir::S);
        }
    }
}

fn charanm_test_animate_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<(&mut TempCharAnimationState, &mut TextureAtlasSprite)>,
    time: Res<Time>,
) {
    for (
        mut state,
        mut sprite,
    ) in query.iter_mut() {
        if let Some(animation) = animations.get(&state.animation) {
            if let Some(variant_name) = &state.variant {
                // get the stugff
                let variant = animation.variants.get(variant_name).unwrap();

                // update the timer... or initialize it, if it's missing.
                if let Some(frame_timer) = &mut state.frame_timer {
                    frame_timer.tick(time.delta());
                    if frame_timer.finished() {
                        // increment+loop frame, and replace the timer with the new frame's duration
                        // TODO: we're only looping rn.
                        let frame_count = variant.frames.len();
                        state.frame = (state.frame + 1) % frame_count;

                        state.frame_timer = Some(Timer::new(variant.frames[state.frame].duration, false));
                    }
                } else {
                    // must be new here. initialize the timer w/ the current
                    // frame's duration, can start ticking on the next loop.
                    let frame = &variant.frames[state.frame];
                    state.frame_timer = Some(Timer::new(frame.duration, false));
                }

                // ok, where was I. Uh, dig into the variant and frame to see
                // what actual texture index we oughtta use, and set it.
                let frame = &variant.frames[state.frame];
                if sprite.index != frame.index {
                    // println!("Updating sprite index! (animating)");
                    sprite.index = frame.index;
                }
            }
        }
    }
}

fn charanm_test_atlas_reassign_system(
    mut commands: Commands,
    animations: Res<Assets<CharAnimation>>,
    query: Query<(Entity, &TempCharAnimationState, &Handle<TextureAtlas>)>,
) {
    for (
            entity,
            state,
            atlas_handle,
        ) in query.iter() {
        // get the animation, get the handle off it, compare the handles, and if
        // they don't match, issue an insert command.
        if let Some(animation) = animations.get(&state.animation) {
            let desired_atlas_handle = &animation.texture_atlas;
            if desired_atlas_handle != atlas_handle {
                println!("Replacing texture handle!");
                commands.entity(entity).insert(desired_atlas_handle.clone());
            }
        }
    }
}

fn charanm_test_setup_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let anim_handle: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRun.aseprite");
    commands
        .spawn_bundle(SpriteSheetBundle {
            transform: Transform::from_translation(Vec3::new(30.0, 60.0, 3.0)),
            ..default()
        })
        .insert(TempCharAnimationState::new(anim_handle, Dir::W, true));

    let test_texture_handle: Handle<Image> = asset_server.load("sprites/sPlayerRun.aseprite#texture");
    commands.spawn_bundle(SpriteBundle {
        texture: test_texture_handle,
        transform: Transform::from_translation(Vec3::new(10.0, 10.0, 3.0)),
        ..default()
    });
}


pub struct TestCharAnimationPlugin;

impl Plugin for TestCharAnimationPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_startup_system(charanm_test_setup_system)
        .add_system(charanm_test_atlas_reassign_system)
        .add_system(charanm_test_animate_system)
        .add_system(charanm_test_directions_system);
    }
}
