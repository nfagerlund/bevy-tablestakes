use asefile::AsepriteFile;
use bevy::asset::{AssetLoader, AssetServer, BoxedFuture, Handle, LoadContext, LoadedAsset};
use bevy::math::{prelude::*, Affine2, Rect};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::{
    render_resource::{Extent3d, TextureDimension, TextureFormat},
    texture::{Image, TextureFormatPixelInfo},
};
use bevy::sprite::{Anchor, TextureAtlas};
use bevy::utils::Duration;
use image::RgbaImage;
use std::collections::HashMap;

use crate::compass::{self, Dir};
use crate::Motion;
use crate::Walkbox;

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

impl CharAnimationVariant {
    /// Returns the final frame time to use for the requested frame, after
    /// applying the specified FrameTimeOverride behavior.
    pub fn resolved_frame_time(
        &self,
        frame_index: usize,
        override_type: FrameTimeOverride,
    ) -> Duration {
        let asset_duration = self.frames[frame_index].duration;
        match override_type {
            FrameTimeOverride::None => asset_duration,
            FrameTimeOverride::Ms(millis) => Duration::from_millis(millis),
            FrameTimeOverride::Scale(scale) => asset_duration.mul_f32(scale),
            FrameTimeOverride::TotalMs(total_millis) => {
                let millis = total_millis / (self.frames.len() as u64); // nearest ms is fine
                Duration::from_millis(millis)
            },
        }
    }
}

// I've changed my mind about tags: I'm gonna start out with a rigid enum,
// enforce tag names at load time, and expand behavior as needed later. Duping
// the Dir type for easier refactors later??
pub type VariantName = compass::Dir;
type VariantsMap = HashMap<VariantName, CharAnimationVariant>;

/// Data for an individual animation frame. This struct contains coordinates for
/// some points and rectangles. The points have some particular frame of
/// reference (described in comments), but the rectangles are all relative to
/// the origin point and laid out in Bevy spatial coordinate space (y-up).
#[derive(Debug)]
pub struct CharAnimationFrame {
    /// Index into the `TextureAtlas`.
    pub index: usize,
    /// Frame duration.
    pub duration: Duration,
    /// Origin point of the sprite in pixel coordinates, where 0,0 is top left.
    /// Stored on the frame for convenience, but shouldn't vary.
    pub origin: Vec2,
    /// Anchor point that can be assigned to a TextureAtlasSprite. Same as
    /// origin, but transformed to normalized coordinates relative to the
    /// texture size (where 0,0 is the center and -.5,-.5 is bottom left).
    pub anchor: Vec2,
    /// Bbox for the projected foot position on the ground.
    pub walkbox: Option<Rect>,
    // todo: hitbox, hurtbox,
}

#[derive(SystemLabel)]
pub struct CharAnimationSystems;
#[derive(SystemLabel)]
pub struct SpriteChangers;

pub struct CharAnimationPlugin;

impl Plugin for CharAnimationPlugin {
    fn build(&self, app: &mut App) {
        let systems = SystemSet::new()
            .label(CharAnimationSystems)
            .after(SpriteChangers)
            .with_system(charanm_atlas_reassign_system)
            .with_system(charanm_set_directions_system)
            .with_system(charanm_animate_system.after(charanm_set_directions_system))
            .with_system(charanm_update_walkbox_system.after(charanm_animate_system));

        app.add_asset::<CharAnimation>()
            .init_asset_loader::<CharAnimationLoader>()
            .add_event::<AnimateFinishedEvent>()
            // These systems should run after any app code that might mutate
            // CharAnimationState or Motion. And set_directions might have
            // mutated the animation state, so that should take effect before
            // the main animate system.
            .add_system_set(systems);
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

const REFLECT_Y_COMPONENTS: [f32; 4] = [1.0, 0.0, 0.0, -1.0];
const REFLECT_Y: Mat2 = Mat2::from_cols_array(&REFLECT_Y_COMPONENTS);
const OFFSET_TO_CENTER: Vec2 = Vec2::new(-0.5, 0.5);

/// Loads an aseprite file and uses it to construct a sprite sheet `#texture`, a
/// `#texture_atlas` that indexes into that sprite sheet, and a top-level
/// `CharAnimation`. The individual `CharAnimationFrames` in the
/// `CharAnimationVariants` contain indexes into the `TextureAtlas`.
/// Assumptions:
/// - File only uses AnimationDirection::Forward.
/// - Tag names are unique in the file. (Aseprite doesn't guarantee this.)
/// - Named tags cover all of the needed animation frames.
///   - OR: there are zero tags and thus only one orientation.
/// - Walkbox layer: "walkbox"
/// - Origin layer: "origin"
/// - Layers for drawn-on metadata coordinates should be marked as invisible in
///   the saved file.
fn load_aseprite(bytes: &[u8], load_context: &mut LoadContext) -> anyhow::Result<()> {
    let ase = AsepriteFile::read(bytes)?;
    let width = ase.width();
    let height = ase.height();
    let num_frames = ase.num_frames();

    // Build an affine transform for converting the pixel origin point to a
    // relative anchor position.
    let texture_size = Vec2::new(width as f32, height as f32);
    let normalize_scale = Mat2::from_diagonal(texture_size).inverse();
    let anchor_transform =
        Affine2::from_mat2_translation(normalize_scale * REFLECT_Y, OFFSET_TO_CENTER);

    // Build the texture atlas, ensuring that its sub-texture indices match the
    // original Aseprite file's frame indices.
    let frame_images: Vec<Image> = (0..num_frames)
        .map(|i| remux_image(ase.frame(i).image()))
        .collect();
    // Atlas will be a 1D horizontal strip w/ 1px padding between frames.
    let atlas_height = height as u32;
    let atlas_width = width as u32 * num_frames + num_frames - 1;
    let mut atlas_texture = Image::new_fill(
        Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],                 // clear
        TextureFormat::Rgba8UnormSrgb, // Could frame_images[0].format(), but hardcode for now.
    );
    // copy time
    let mut cur_x = 0_usize;
    for img in frame_images.iter() {
        copy_texture_to_atlas(&mut atlas_texture, img, width, height, cur_x, 0);
        cur_x += width + 1;
    }
    // stow texture and get a handle
    let texture_handle = load_context.set_labeled_asset("texture", LoadedAsset::new(atlas_texture));
    // atlas time!!!
    // N.b.: from_grid_with_padding adds grid cells in left-to-right,
    // top-to-bottom order, and we rely on this to make the frame indices match.
    let atlas = TextureAtlas::from_grid(
        texture_handle,
        Vec2::new(width as f32, height as f32),
        num_frames as usize,
        1,
        Some(Vec2::new(1.0, 0.0)),
        None,
    );
    let atlas_handle = load_context.set_labeled_asset("texture_atlas", LoadedAsset::new(atlas));

    // Since our final frame indices are reliable, processing tags is easy.
    let mut variants: VariantsMap = HashMap::new();

    // Closure for the heavy lifting (since we can't handle some tags / 0 tags
    // in the same for-loop):
    let mut process_frame_range =
        |name: VariantName, frame_range: core::ops::RangeInclusive<u32>| {
            let frames: Vec<CharAnimationFrame> = frame_range
                .map(|i| {
                    let frame = ase.frame(i);
                    let index = i as usize;
                    let duration_ms = frame.duration() as u64;
                    let duration = Duration::from_millis(duration_ms);

                    // Wasteful, bc we could exit early on first non-clear px, but meh.
                    let origin = match rect_from_cel(&ase, "origin", i) {
                        Some(origin_rect) => origin_rect.min,
                        None => Vec2::ZERO, // Origin's non-optional.
                    };

                    // Get the walkbox, offset it relative to the origin, THEN flip the Y.
                    let absolute_walkbox = rect_from_cel(&ase, "walkbox", i);
                    let walkbox = absolute_walkbox.map(|wbox| {
                        let relative_walkbox = Rect {
                            min: wbox.min - origin,
                            max: wbox.max - origin,
                        };
                        flip_rect_y(relative_walkbox)
                    });

                    let anchor = anchor_transform.transform_point2(origin);

                    CharAnimationFrame {
                        index,
                        duration,
                        origin,
                        anchor,
                        walkbox,
                    }
                })
                .collect();

            let variant = CharAnimationVariant { name, frames };
            variants.insert(name, variant);
        };

    if ase.num_tags() == 0 {
        // then treat whole file as one variant.
        let frame_range = 0..=(ase.num_frames() - 1);
        process_frame_range(VariantName::Neutral, frame_range);
    } else {
        // one variant per tag.
        for tag in (0..ase.num_tags()).map(|i| ase.tag(i)) {
            let name: VariantName = tag.name().try_into()?; // Just propagate error, don't continue load.
            let frame_range = tag.from_frame()..=tag.to_frame(); // inclusive
            process_frame_range(name, frame_range);
        }
    }

    // The whole enchilada:
    let animation = CharAnimation {
        variants,
        texture_atlas: atlas_handle,
    };

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
        depth_or_array_layers: 1,
    };
    Image::new(
        size,
        TextureDimension::D2,
        img.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
    )
}

/// Invert the Y coordinates of a Vec2, because source image coordinates go
/// Y-down but bevy spatial coordinates go Y-up. Note that this is only valid to
/// do if you're not also moving the origin point!
fn invert_vec2_y(v: Vec2) -> Vec2 {
    Vec2::new(v.x, -(v.y))
}

/// OK, rects have "min" (smallest x, smallest y) and "max" (largest x, largest
/// y) corners. When you create a rect within a top-down Y coordinate system,
/// "min" is the top left, and "max" is the bottom right. But if you want to
/// represent the same rectangle in a bottom-up Y coordinate system, "min" is
/// the BOTTOM left, and "max" is the TOP right. So it's not just inverting the
/// Y coordinate of each point, you also have to recombine them. If you don't,
/// it ends up inside-out.
fn flip_rect_y(r: Rect) -> Rect {
    let top_left = r.min;
    let bottom_right = r.max;
    let bottom_left = Vec2::new(top_left.x, bottom_right.y);
    let top_right = Vec2::new(bottom_right.x, top_left.y);
    Rect {
        min: invert_vec2_y(bottom_left),
        max: invert_vec2_y(top_right),
    }
}

/// Get the bounding Rect for a cel's non-transparent pixels.
fn rect_from_cel(ase: &AsepriteFile, layer_name: &str, frame_index: u32) -> Option<Rect> {
    ase.layer_by_name(layer_name)
        .map(|layer| {
            let cel_img = layer.frame(frame_index).image();
            get_rect_lmao(&cel_img)
        })
        .flatten()
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
            if x < x_min {
                x_min = x;
            }
            if x > x_max {
                x_max = x;
            }
            if y < y_min {
                y_min = y;
            }
            if y > y_max {
                y_max = y;
            }
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
        atlas_texture.data[begin..end].copy_from_slice(&texture.data[texture_begin..texture_end]);
    }
}

// Right, so, how do we animate these dudes? (and thus, what do we need in our state component/bundle?)
// Starting in media res:
// - Each step, tick down a TIMER (non-repeating) for the current frame.
// - When the timer runs out, switch your FRAME INDEX to the next frame (or, WRAP AROUND if you're configured to loop).

pub struct AnimateFinishedEvent(pub Entity);

#[derive(Component, Debug)]
pub struct CharAnimationState {
    pub animation: Handle<CharAnimation>,
    pub variant: Option<VariantName>,
    pub playback: Playback,
    pub frame: usize,
    // To start with, we'll just always loop.
    pub frame_timer: Option<Timer>,
    /// Optionally override the animation's frame timings, setting every frame
    /// to the provided length in milliseconds. If you need frames to be
    /// different lengths, that belongs in the animation data, not the
    pub frame_time_override: FrameTimeOverride,
}

#[derive(Debug, Clone, Copy)]
pub enum Playback {
    Loop,
    Once,
}

/// Allow programmatically overriding the frame times from the animation source
/// data, for things like stretching out a motion to fit it to a particular
/// total duration.
#[derive(Debug, Clone, Copy)]
pub enum FrameTimeOverride {
    None,
    Ms(u64),
    Scale(f32),
    TotalMs(u64),
}

impl CharAnimationState {
    pub fn new(animation: Handle<CharAnimation>, variant: VariantName, playback: Playback) -> Self {
        CharAnimationState {
            animation,
            variant: Some(variant),
            playback,
            // in the future I might end up wanting to blend between animations
            // at a particular frame. Doesn't matter yet tho.
            frame: 0,
            frame_timer: None,
            frame_time_override: FrameTimeOverride::None,
        }
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.frame_timer = None;
        self.frame_time_override = FrameTimeOverride::None;
    }

    /// Change direction of animation, unless already facing the requested direction.
    pub fn change_variant(&mut self, variant: VariantName) {
        if self.variant != Some(variant) {
            self.variant = Some(variant);
            self.reset();
        }
    }

    pub fn change_animation(&mut self, animation: Handle<CharAnimation>, playback: Playback) {
        if self.animation != animation {
            self.animation = animation;
            // TODO: I'm leaving the variant the same, but actually I don't know if
            // that's the right move -- question is whether I'd ever switch to an
            // animation with fewer variants.
            self.playback = playback;
            self.reset();
        }
    }

    pub fn _set_frame_times_to(&mut self, millis: u64) {
        self.frame_time_override = FrameTimeOverride::Ms(millis);
    }

    pub fn _scale_frame_times_by(&mut self, scale: f32) {
        self.frame_time_override = FrameTimeOverride::Scale(scale);
    }

    pub fn set_total_run_time_to(&mut self, millis: u64) {
        self.frame_time_override = FrameTimeOverride::TotalMs(millis);
    }

    pub fn timer_just_finished(&self) -> bool {
        if let Some(true) = self.frame_timer.as_ref().map(|t| t.just_finished()) {
            true
        } else {
            false
        }
    }
}

fn charanm_set_directions_system(mut query: Query<(&mut CharAnimationState, &Motion)>) {
    for (mut state, motion) in query.iter_mut() {
        // just doing this unconditionally and letting change_variant sort it out.
        let dir = Dir::cardinal_from_angle(motion.facing);
        state.change_variant(dir);
    }
}

pub fn charanm_animate_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<(&mut CharAnimationState, &mut TextureAtlasSprite, Entity)>,
    time: Res<Time>,
    mut finished_events: EventWriter<AnimateFinishedEvent>,
) {
    for (mut state, mut sprite, entity) in query.iter_mut() {
        let Some(animation) = animations.get(&state.animation) else { continue; };
        let Some(variant_name) = &state.variant else { continue; };
        // get the stugff
        let Some(variant) = animation.variants.get(variant_name) else { continue; };

        let mut updating_frame = false;

        // update the timer... or initialize it, if it's missing.
        if let Some(frame_timer) = &mut state.frame_timer {
            frame_timer.tick(time.delta());
            'timers: while state.timer_just_finished() {
                // Determine the next frame
                let frame_count = variant.frames.len();
                let next_frame = (state.frame + 1) % frame_count;

                // If next is 0, we just finished the *last* frame... fire an
                // event in case anyone wants to do something about that. This
                // is valid for single-frame animations too, although it might
                // not seem it at first blush.
                if next_frame == 0 {
                    finished_events.send(AnimateFinishedEvent(entity));
                    // If this is a non-looping animation, we bail now and leave
                    // it perma-stuck on the final frame. Its timer will keep
                    // accumulating, and this loop won't run again until the
                    // animation is changed.
                    match state.playback {
                        Playback::Once => {
                            break 'timers;
                        },
                        // nothing interesting yet for looping animations, but I
                        // want the exhaustiveness check from `match` just in case.
                        Playback::Loop => (),
                    }
                }

                updating_frame = true;
                let excess_time = state.frame_timer.as_ref().unwrap().excess_elapsed();

                // increment+loop frame, and replace the timer with the new frame's duration
                state.frame = next_frame;
                let duration = variant.resolved_frame_time(state.frame, state.frame_time_override);
                let mut new_timer = Timer::new(duration, TimerMode::CountUp);
                new_timer.tick(excess_time);
                state.frame_timer = Some(new_timer);
            }
        } else {
            // must be new here. initialize the timer w/ the current
            // frame's duration, can start ticking on the next loop.
            updating_frame = true;
            let duration = variant.resolved_frame_time(state.frame, state.frame_time_override);
            state.frame_timer = Some(Timer::new(duration, TimerMode::CountUp));
        }

        // ok, where was I.
        if updating_frame {
            // Uh, dig into the variant and frame to see what actual
            // texture index we oughtta use, and set it.
            let frame = &variant.frames[state.frame];
            sprite.index = frame.index;
            // Also, set the origin:
            sprite.anchor = Anchor::Custom(frame.anchor);
            // But leave colliders to their own systems.
        }
    }
}

/// The main animate system updates the origin because everything's gotta have
/// one, but maybe not everything needs a collider, even if its sprite has one.
/// So, we only update walkboxes for entities who have opted in by having one
/// added to them at some point, and we never remove walkboxes. BTW: We're
/// filtering on Changed<TextureAtlasSprite> because the animation state
/// contains a Timer and thus changes constantly... but we only update the atlas
/// index when it's time to flip frames.
fn charanm_update_walkbox_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<(&CharAnimationState, &mut Walkbox), Changed<TextureAtlasSprite>>,
) {
    for (state, mut walkbox) in query.iter_mut() {
        let Some(animation) = animations.get(&state.animation) else { continue; };
        let Some(variant_name) = &state.variant else { continue; };
        let Some(variant) = animation.variants.get(variant_name) else { continue; };
        let frame = &variant.frames[state.frame];

        // If there's no walkbox in the frame, you get a 0-sized rectangle at your origin.
        let sprite_walkbox = frame.walkbox.unwrap_or_default();
        walkbox.0 = sprite_walkbox;
    }
}

fn charanm_atlas_reassign_system(
    animations: Res<Assets<CharAnimation>>,
    mut query: Query<(&CharAnimationState, &mut Handle<TextureAtlas>)>,
) {
    for (state, mut atlas_handle) in query.iter_mut() {
        // get the animation, get the handle off it, compare the handles, and if
        // they don't match, replace the value.
        if let Some(animation) = animations.get(&state.animation) {
            let desired_atlas_handle = &animation.texture_atlas;
            if *desired_atlas_handle != *atlas_handle {
                *atlas_handle = desired_atlas_handle.clone();
            }
        }
    }
}

// Follow the birdie
fn charanm_test_set_motion_system(
    mut query: Query<&mut Motion, With<Goofus>>,
    inputs: Res<crate::input::CurrentInputs>,
) {
    for mut motion in query.iter_mut() {
        motion.face(inputs.movement * -1.0);
    }
}

fn charanm_test_setup_system(mut commands: Commands, asset_server: Res<AssetServer>) {
    let anim_handle: Handle<CharAnimation> = asset_server.load("sprites/sPlayerRun.aseprite");
    commands.spawn((
        Goofus,
        SpriteSheetBundle {
            transform: Transform::from_translation(Vec3::new(30.0, 60.0, 3.0)),
            ..default()
        },
        crate::render::HasShadow,
        CharAnimationState::new(anim_handle, Dir::W, Playback::Loop),
        Motion::new(Vec2::ZERO),
    ));

    let test_texture_handle: Handle<Image> =
        asset_server.load("sprites/sPlayerRun.aseprite#texture");
    commands.spawn(SpriteBundle {
        texture: test_texture_handle,
        transform: Transform::from_translation(Vec3::new(10.0, 10.0, 3.0)),
        ..default()
    });
}

#[derive(Component)]
struct Goofus;

pub struct TestCharAnimationPlugin;

impl Plugin for TestCharAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(charanm_test_setup_system)
            .add_system(charanm_test_set_motion_system);
    }
}
