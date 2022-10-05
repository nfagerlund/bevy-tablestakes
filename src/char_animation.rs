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
use bevy_inspector_egui::Inspectable;
use bevy_inspector_egui::egui::Key;
use std::collections::HashMap;
use std::env::var;
use std::f32::consts::*;
use asefile::AsepriteFile;
use image::RgbaImage;

#[derive(Debug, TypeUuid)]
#[uuid = "585e2e41-4a97-42ef-a13e-55761c854bb4"]
pub struct CharAnimation {
    pub variants: HashMap<String, CharAnimationVariant>,
    pub texture_atlas: Handle<TextureAtlas>,
}

#[derive(Debug)]
pub struct CharAnimationVariant {
    pub name: String,
    pub frames: Vec<CharAnimationFrame>,
}

/// Data for an individual animation frame. This struct contains coordinates for
/// some points and rectangles; in all cases, these are relative to the top left
/// corner of the sprite.
#[derive(Debug)]
pub struct CharAnimationFrame {
    /// The index to use for getting this frame from this animation's
    /// `TextureAtlas`. This can be assigned to a `TextureAtlasSprite` when it's
    /// time to display this frame.
    pub index: usize,
    /// How long this frame should be displayed for, according to the animation
    /// source file.
    pub duration: Duration,
    /// The origin/anchor/pivot point of the sprite. Technically I expect the
    /// origin not to differ across frames of the same animation variant, but
    /// it's stored on the frame for convenience.
    pub origin: Vec2,
    /// The bounding box that represents the projected position of the
    /// character's feet onto the ground, for determining where they're standing
    /// or trying to stand.
    pub walkbox: Option<Rect>,
    // hitbox, hurtbox,
}

/// Right, so, how do we animate these dudes? (and thus, what do we need in our state component/bundle?)
/// Starting in media res:
/// - Each step, tick down a TIMER (non-repeating) for the current frame.
/// - When the timer runs out, switch your FRAME INDEX to the next frame (or, WRAP AROUND if you're configured to loop).

#[derive(Component, Debug)]
pub struct TempCharAnimationState {
    animation: Handle<CharAnimation>,
    variant: Option<String>, // hate the string lookup here btw, need something better.
    // guess I could do interning and import IndexMap maybe.
    frame: usize,
    // To start with, we'll just always loop.
    frame_timer: Option<Timer>,
}

impl TempCharAnimationState {
    fn new(animation: Handle<CharAnimation>, variant: &str) -> Self {
        TempCharAnimationState {
            animation,
            variant: Some(variant.to_string()),
            // in the future I might end up wanting to blend between animations
            // at a particular frame. Doesn't matter yet tho.
            frame: 0,
            frame_timer: None,
        }
    }

    fn change_variant(&mut self, variant: &str) {
        self.variant = Some(variant.to_string());
        self.frame = 0;
        self.frame_timer = None;
    }
}

pub struct CharAnimationPlugin;

impl Plugin for CharAnimationPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_asset::<CharAnimation>()
            .init_asset_loader::<CharAnimationLoader>()
            .add_startup_system(charanm_test_setup_system)
            .add_system(charanm_test_atlas_reassign_system)
            .add_system(charanm_test_animate_system)
            .add_system(charanm_test_directions_system);
    }
}

fn charanm_test_directions_system(
    mut query: Query<&mut TempCharAnimationState>,
    keys: Res<Input<KeyCode>>,
) {
    for mut state in query.iter_mut() {
        if keys.just_pressed(KeyCode::Right) {
            state.change_variant("E");
        } else if keys.just_pressed(KeyCode::Up) {
            state.change_variant("N");
        } else if keys.just_pressed(KeyCode::Left) {
            state.change_variant("W");
        } else if keys.just_pressed(KeyCode::Down) {
            state.change_variant("S");
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
            // UGH!!!
            if let Some(variant_name) = &state.variant {
                // get the stugff
                let variant = animation.variants.get(variant_name).unwrap(); // UGH!!

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
        .insert(TempCharAnimationState::new(anim_handle, "W"));

    let test_texture_handle: Handle<Image> = asset_server.load("sprites/sPlayerRun.aseprite#texture");
    commands.spawn_bundle(SpriteBundle {
        texture: test_texture_handle,
        transform: Transform::from_translation(Vec3::new(10.0, 10.0, 3.0)),
        ..default()
    });
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
    let mut variants: HashMap<String, CharAnimationVariant> = HashMap::new();

    for tag in (0..ase.num_tags()).map(|i| ase.tag(i)) {
        let name = tag.name().to_string();
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

        let variant = CharAnimationVariant { name: name.clone(), frames };
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

/// Extract the non-transparent-pixels bounding rectangle from the cel at a
/// given layer name and frame index.
fn rect_from_cel(ase: &AsepriteFile, layer_name: &str, frame_index: u32) -> Option<Rect> {
    ase.layer_by_name(layer_name).map(|layer| {
        let cel_img = layer.frame(frame_index).image();
        get_rect_lmao(&cel_img)
    }).flatten()
}

/// Find the Rect that contains all the non-transparent pixels in a cel.
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

// Yoinked+hacked fn from TextureAtlasBuilder (which I can't use directly
// because it relies on runtime asset collections):
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

#[derive(Debug, PartialEq, Eq)]
enum DiscreteDir {
    E, N, W, S,
    NE, NW, SW, SE,
    Neutral,
}

impl DiscreteDir {
    // Given a Vec2, return east, west, or neutral. Bias towards east.
    fn horizontal_from_vec2(motion: Vec2) -> Self {
        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            Self::Neutral
        } else if motion.x < 0.0 {
            Self::W
        } else {
            Self::E
        }
    }

    // Given a Vec2, return north, south, or neutral. Bias towards south.
    fn vertical_from_vec2(motion: Vec2) -> Self {
        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            Self::Neutral
        } else if motion.y > 0.0 {
            Self::N
        } else {
            Self::S
        }
    }

    // Given a Vec2, return one of the four cardinal directions or neutral. Bias
    // towards horizontal.
    fn cardinal_from_vec2(motion: Vec2) -> Self {
        const NE: f32 = FRAC_PI_4;
        const NW: f32 = 3.0 * FRAC_PI_4;
        const SW: f32 = -3.0 * FRAC_PI_4;
        const SE: f32 = -FRAC_PI_4;

        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            return Self::Neutral;
        }
        let angle = Vec2::X.angle_between(motion);
        if angle >= SE && angle <= NE {
            Self::E
        } else if angle > NE && angle < NW {
            Self::N
        } else if angle >= NW || angle <= SW { // the negative flip-over
            Self::W
        } else if angle > SW && angle < SE {
            Self::S
        } else {
            panic!("IDK what happened, but some angle didn't match a dir: {}", angle)
        }
    }

    // Given a Vec2, return one of the four cardinal directions, one of the four
    // ordinal/intercardinal directions, or neutral. Bias is whatever bc you
    // can't get your analog inputs exact enough to notice it.
    fn cardinal_ordinal_from_vec2(motion: Vec2) -> Self {
        const ENE: f32 = 1.0 * FRAC_PI_8;
        const NNE: f32 = 3.0 * FRAC_PI_8;
        const NNW: f32 = 5.0 * FRAC_PI_8;
        const WNW: f32 = 7.0 * FRAC_PI_8;
        const WSW: f32 = -7.0 * FRAC_PI_8;
        const SSW: f32 = -5.0 * FRAC_PI_8;
        const SSE: f32 = -3.0 * FRAC_PI_8;
        const ESE: f32 = -1.0 * FRAC_PI_8;
        // Deal with any tricksy infinite or NaN vectors:
        let motion = motion.normalize_or_zero();
        if motion == Vec2::ZERO {
            return Self::Neutral;
        }
        let angle = Vec2::X.angle_between(motion);
        if angle > ESE && angle <= ENE {
            Self::E
        } else if angle > ENE && angle <= NNE {
            Self::NE
        } else if angle > NNE && angle <= NNW {
            Self::N
        } else if angle > NNW && angle <= WNW {
            Self::NW
        } else if angle > WNW || angle <= WSW { // the negative flip-over
            Self::W
        } else if angle > WSW && angle <= SSW {
            Self::SW
        } else if angle > SSW && angle <= SSE {
            Self::S
        } else if angle > SSE && angle <= ESE {
            Self::SE
        } else {
            panic!("IDK what happened, but some angle didn't match a dir: {}", angle)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HARD_NE: Vec2 = Vec2::new(1.0, 1.0);
    const HARD_NW: Vec2 = Vec2::new(-1.0, 1.0);
    const HARD_SE: Vec2 = Vec2::new(1.0, -1.0);
    const HARD_SW: Vec2 = Vec2::new(-1.0, -1.0);

    const LIL_BIT: f32 = 0.0001;

    #[test]
    fn test_horizontal_from_vec2() {
        assert_eq!(DiscreteDir::horizontal_from_vec2(HARD_NE), DiscreteDir::E);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(LIL_BIT, 1.0)), DiscreteDir::E);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(-LIL_BIT, 1.0)), DiscreteDir::W);
        // on the deciding line:
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(0.0, 1.0)), DiscreteDir::E);
        // Blank or bogus input:
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::horizontal_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }

    #[test]
    fn test_vertical_from_vec2() {
        assert_eq!(DiscreteDir::vertical_from_vec2(HARD_NE), DiscreteDir::N);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(1.0, LIL_BIT)), DiscreteDir::N);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(1.0, -LIL_BIT)), DiscreteDir::S);
        // on the deciding line:
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(-1.0, 0.0)), DiscreteDir::S);
        // Blank or bogus input:
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::vertical_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }

    #[test]
    fn test_cardinal_from_vec2() {
        // Truly easy cases:
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, 0.0)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, LIL_BIT)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, -LIL_BIT)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-1.0, 0.0)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-1.0, LIL_BIT)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-1.0, -LIL_BIT)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(0.0, 1.0)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(LIL_BIT, 1.0)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-LIL_BIT, 1.0)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(0.0, -1.0)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(LIL_BIT, -1.0)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-LIL_BIT, -1.0)), DiscreteDir::S);

        // Clear-cut cases (inter-intercardinal):
        // inter-intercardinal x/y components
        let iic_short: f32 = FRAC_PI_8.sin();
        let iic_long: f32 = FRAC_PI_8.cos();
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_long, iic_short)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_short, iic_long)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_short, iic_long)), DiscreteDir::N);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_long, iic_short)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_long, -iic_short)), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(-iic_short, -iic_long)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_short, -iic_long)), DiscreteDir::S);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(iic_long, -iic_short)), DiscreteDir::E);

        // Edge cases (hard ordinals):
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_NE), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_NW), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_SW), DiscreteDir::W);
        assert_eq!(DiscreteDir::cardinal_from_vec2(HARD_SE), DiscreteDir::E);

        // Blank or bogus input:
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }

    #[test]
    fn test_cardinal_ordinal_from_vec2() {
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_NE), DiscreteDir::NE);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_NW), DiscreteDir::NW);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_SE), DiscreteDir::SE);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(HARD_SW), DiscreteDir::SW);

        // inter-intercardinal x/y components
        let iic_short: f32 = FRAC_PI_8.sin();
        let iic_long: f32 = FRAC_PI_8.cos();

        // On _just_ one side or the other of the deciding line:
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long + LIL_BIT, iic_short)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long, iic_short + LIL_BIT)), DiscreteDir::NE);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long + LIL_BIT, -iic_short)), DiscreteDir::E);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long, -(iic_short + LIL_BIT))), DiscreteDir::SE);

        // On exactly the deciding line:
        match DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(iic_long, iic_short)) {
            DiscreteDir::E => (),
            DiscreteDir::NE => (),
            _ => {
                panic!("pi/8 angle should be either E or NE (don't care which)");
            }
        }

        // Blank or bogus input:
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::ZERO), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(f32::NAN, 1.0)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(1.0, f32::INFINITY)), DiscreteDir::Neutral);
        assert_eq!(DiscreteDir::cardinal_ordinal_from_vec2(Vec2::new(f32::NEG_INFINITY, 1.0)), DiscreteDir::Neutral);
    }
}
