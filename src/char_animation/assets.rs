//! Asset loader functionality, for juicing aseprite files into a
//! combination of texture atlases and animation data. This is the messiest
//! stuff in this module, so it's very nice to have it isolated.

use super::types::*;
use crate::toolbox::{flip_rect_y, move_rect_origin};

use asefile::AsepriteFile;
use bevy::asset::AsyncReadExt;
use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::math::{prelude::*, Affine2, Rect};
use bevy::render::{
    render_asset::RenderAssetUsages,
    render_resource::{Extent3d, TextureDimension, TextureFormat},
    texture::{Image, TextureFormatPixelInfo},
};
use bevy::sprite::TextureAtlasLayout;
use bevy::utils::Duration;
use image::RgbaImage;
use std::collections::HashMap;

#[derive(Default)]
pub struct CharAnimationLoader;

impl AssetLoader for CharAnimationLoader {
    type Asset = CharAnimation;
    type Settings = ();
    type Error = anyhow::Error;

    fn extensions(&self) -> &[&str] {
        &["aseprite", "ase"]
    }

    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        load_aseprite(&bytes, load_context)
    }
}

const REFLECT_Y_COMPONENTS: [f32; 4] = [1.0, 0.0, 0.0, -1.0];
const REFLECT_Y: Mat2 = Mat2::from_cols_array(&REFLECT_Y_COMPONENTS);
const OFFSET_TO_CENTER: Vec2 = Vec2::new(-0.5, 0.5);

/// Loads an aseprite file and uses it to construct a sprite sheet `#texture`, a
/// `#texture_atlas_layout` that indexes into that sprite sheet, and a top-level
/// `CharAnimation`. The individual `CharAnimationFrames` in the
/// `CharAnimationVariants` contain indexes into the `TextureAtlas`.
/// Assumptions:
/// - File only uses AnimationDirection::Forward.
/// - Tag names are unique in the file. (Aseprite doesn't guarantee this.)
/// - Named tags cover all of the needed animation frames.
///   - OR: there are zero tags and thus only one orientation.
/// - Walkbox layer: "walkbox"
/// - Hitbox layer: "hitbox"
/// - Hurtbox layer: "hurtbox"
/// - Origin layer: "origin"
/// - Layers for drawn-on metadata coordinates should be marked as invisible in
///   the saved file.
fn load_aseprite(bytes: &[u8], load_context: &mut LoadContext) -> anyhow::Result<CharAnimation> {
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

    // ~~ #texture ~~
    // Capture the handle for the next step.
    let texture_handle = load_context.labeled_asset_scope("texture".to_string(), |_lc| -> Image {
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
            RenderAssetUsages::default(),
        );
        // copy time
        let mut cur_x = 0_usize;
        for img in frame_images.iter() {
            copy_texture_to_atlas(&mut atlas_texture, img, width, height, cur_x, 0);
            cur_x += width + 1;
        }
        // return!
        atlas_texture
    });

    // ~~ #texture_atlas_layout ~~
    let atlas_layout_handle = load_context.labeled_asset_scope(
        "texture_atlas_layout".to_string(),
        |_lc| -> TextureAtlasLayout {
            // N.b.: from_grid adds grid cells in left-to-right,
            // top-to-bottom order, and we rely on this to make the frame indices match.
            // capture handle for later
            TextureAtlasLayout::from_grid(
                Vec2::new(width as f32, height as f32),
                num_frames as usize,
                1,
                Some(Vec2::new(1.0, 0.0)),
                None,
            )
        },
    );

    // Since our final frame indices are reliable, processing tags is easy.
    let mut variants: VariantsMap = HashMap::new();

    // Closure for the heavy lifting (since we can't handle some tags / 0 tags
    // in the same for-loop):
    let mut process_frame_range =
        |name: VariantName, frame_range: core::ops::RangeInclusive<u32>| {
            let mut total_duration = Duration::default();
            let frames: Vec<CharAnimationFrame> = frame_range
                .map(|i| {
                    let frame = ase.frame(i);
                    let index = i as usize;
                    let duration_ms = frame.duration() as u64;
                    let duration = Duration::from_millis(duration_ms);

                    total_duration += duration;

                    // Wasteful, bc we could exit early on first non-clear px, but meh.
                    let origin = match rect_from_cel(&ase, "origin", i) {
                        Some(origin_rect) => origin_rect.min,
                        None => Vec2::ZERO, // Origin's non-optional.
                    };

                    // Get each box, position it relative to the origin, THEN flip the Y.
                    // (This is because source image coordinates go Y-down, but bevy spatial
                    // coordinates go Y-up.)
                    let walkbox = anchored_physical_rect_from_cel(&ase, "walkbox", i, origin);
                    let hitbox = anchored_physical_rect_from_cel(&ase, "hitbox", i, origin);
                    let hurtbox = anchored_physical_rect_from_cel(&ase, "hurtbox", i, origin);

                    let anchor = anchor_transform.transform_point2(origin);

                    CharAnimationFrame {
                        index,
                        duration,
                        origin,
                        anchor,
                        walkbox,
                        hitbox,
                        hurtbox,
                    }
                })
                .collect();
            let variant = CharAnimationVariant {
                name,
                frames,
                duration: total_duration,
            };
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

    // Determine directionality... maybe pull this out into a function someday
    // Anyway, count em up... but, don't bother implementing directionalities I'm not using yet.
    let directionality = if variants.len() >= 4
        && variants.contains_key(&VariantName::E)
        && variants.contains_key(&VariantName::N)
        && variants.contains_key(&VariantName::W)
        && variants.contains_key(&VariantName::S)
    {
        Directionality::Four
    } else if variants.contains_key(&VariantName::E) {
        Directionality::OneE
    } else {
        Directionality::Zero
    };

    // The whole enchilada:
    let animation = CharAnimation {
        variants,
        directionality,
        layout: atlas_layout_handle,
        texture: texture_handle,
    };

    // And, cut!
    Ok(animation)
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
        RenderAssetUsages::default(),
    )
}

/// Get the bounding Rect for a cel's non-transparent pixels, located relative to
/// a provided origin point (in y-down image coordinates) and then transformed into
/// y-up engine physical coordinates.
fn anchored_physical_rect_from_cel(
    ase: &AsepriteFile,
    layer_name: &str,
    frame_index: u32,
    origin: Vec2,
) -> Option<Rect> {
    rect_from_cel(ase, layer_name, frame_index).map(|r| flip_rect_y(move_rect_origin(r, origin)))
}

/// Get the bounding Rect for a cel's non-transparent pixels.
fn rect_from_cel(ase: &AsepriteFile, layer_name: &str, frame_index: u32) -> Option<Rect> {
    ase.layer_by_name(layer_name).and_then(|layer| {
        let cel_img = layer.frame(frame_index).image();
        get_rect_lmao(&cel_img)
    })
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

// TODO 0.13: maybe actually use TextureAtlasBuilder now. :thonking:
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
