use bevy::asset::{
    AssetIoError, Assets, AssetServer, AssetLoader, AssetPath, BoxedFuture, Handle, LoadContext, LoadedAsset,
};
use bevy::math::prelude::*;
use bevy::prelude::{App, AddAsset, Plugin};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::texture::Image;
use bevy::sprite::{Rect, TextureAtlas, TextureAtlasBuilder};
use bevy::utils::Duration;
use std::collections::HashMap;
use asefile::AsepriteFile;
use image::RgbaImage;

pub struct CharAnimation {
	pub variants: HashMap<String, CharAnimationVariant>,
	pub texture_atlas: Handle<TextureAtlas>,
}

pub struct CharAnimationVariant {
	pub name: String,
	pub origin: Vec2, // Can't vary per frame, picked from first frame
	pub frames: Vec<CharAnimationFrame>,
}

pub struct CharAnimationFrame {
	pub index: usize,
	pub duration: Duration, // ?
	pub walkbox: Option<Rect>,
	// hitbox, hurtbox,
}

struct TempFrame {
	texture_handle: Handle<Image>,
	duration_ms: u32,
	walkbox: Option<Rect>,
}

pub struct CharAnimationPlugin;

impl Plugin for CharAnimationPlugin {
	fn build(&self, app: &mut App) {
		app.init_asset_loader::<CharAnimationLoader>();
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
            // DO SOMETHING HERE w/ ?, returning (), and using LoadContext.set_default_asset for its side effects
			let bevy_img = load_aseprite_badly(bytes)?;
			load_context.set_default_asset(LoadedAsset::new(bevy_img));
            Ok(())
        })
	}
}

// Return the real asset, not ()!
// Oh, lol, this needs to have hellacious side effects, because so does
// TextureAtlasBuilder.
fn load_aseprite(bytes: &[u8], textures: &mut Assets<Image>, texture_atlases: &mut Assets<TextureAtlas>) -> anyhow::Result<CharAnimation> {
    let ase = AsepriteFile::read(bytes)?;
	// Assumptions: I'm only using AnimationDirection::Forward, and I'm assuming
	// tag names are unique in the file (which aseprite doesn't guarantee), and
	// I'm assuming there are named tags that cover all the needed animation.
	let mut atlas_builder = TextureAtlasBuilder::default();
	let mut variants: HashMap<String, CharAnimationVariant> = HashMap::new();

	let mut tag_data: Vec<(String, Vec<TempFrame>)> = Vec::new();
	for tag in (0..ase.num_tags()).map(|i| ase.tag(i)) {
		// Construct a varaiant struct, and add images to atlas builder
		let frame_count = (tag.to_frame() - tag.from_frame() + 1) as usize;
		let mut frames: Vec<CharAnimationFrame> = Vec::with_capacity(frame_count);
		let mut temp_frames: Vec<TempFrame> = Vec::with_capacity(frame_count);

		// something to hold stuff while I'm working on it...
		for frame_index in tag.from_frame()..=tag.to_frame() {
			let frame = ase.frame(frame_index);
			let duration_ms = frame.duration();
			// Get and dispose of frame image
			let frame_img = frame.image();
			let texture = remux_image(frame_img);
			let texture_handle = textures.add(texture);
			let texture = textures.get(&texture_handle).unwrap();
			atlas_builder.add_texture(texture_handle.clone(), texture);
			// Get walkbox
			let walkbox = if let Some(walkbox_layer) = ase.layer_by_name("walkbox") {
				let walkbox_cel = walkbox_layer.frame(frame_index);
				let walkbox_image = walkbox_cel.image();
				find_rectangle_bounds(&walkbox_image)
			} else {
				None
			};
			// Save a temp frame to process later (once we have a texture atlas
			// to read from)
			temp_frames.push(TempFrame {
				texture_handle,
				duration_ms,
				walkbox,
			});
		}

		// Stash tag data bc we can't process it until we have an atlas:
		tag_data.push((tag.name().to_string(), temp_frames));
		// wow, this kinda sucks lmao! ANYWAY
	}

	// Start finishing the whole asset
	let texture_atlas = atlas_builder.finish(textures)?;
	// Convert each tag's temp data into a CharAnimationVariant, and add it to
	// the variants hashmap
	for (name, temp_frames) in tag_data {
		let frames: Vec<CharAnimationFrame> = temp_frames.into_iter().map(|tf| {
			let TempFrame { texture_handle, duration_ms, walkbox } = tf;
			let index = texture_atlas.get_texture_index(&texture_handle).unwrap();
			let duration = Duration::from_millis(duration_ms as u64);
			CharAnimationFrame {
				index,
				duration,
				walkbox,
			}
		}).collect();
		let variant = CharAnimationVariant {
			name: name.clone(),
			origin: Vec2::ZERO, // TODO idek yet
			frames,
		};
		variants.insert(name, variant);
	}

	// Only add the atlas as an asset once we're done looking stuff up in it:
	let atlas_handle = texture_atlases.add(texture_atlas);

	// Okay, FINALLY: return the whole asset
	Ok(CharAnimation {
		variants,
		texture_atlas: atlas_handle,
	})
}

// Test texture creation
fn load_aseprite_badly(bytes: &[u8]) -> anyhow::Result<Image> {
	let ase = AsepriteFile::read(bytes)?;
	let img = ase.frame(0).image();
	let bevy_img = remux_image(img);
	dbg!(&bevy_img);
	Ok(bevy_img)
}

/// Convert the specific variant of `image::ImageBuffer` that
/// `asefile::Frame.image()` returns into a `bevy::render::texture::Image`.
/// Consumes the argument and re-uses the internal container.
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

/// Find the Rect that contains all the non-transparent pixels in a cel.
fn find_rectangle_bounds(img: &RgbaImage) -> Option<Rect> {
    let mut x_min: u32 = u32::MAX;
    let mut x_max: u32 = 0;
    let mut y_min: u32 = u32::MAX;
    let mut y_max: u32 = 0;
	let mut present = false;
    let blank = image::Rgba([0,0,0,0]);
    for (x, y, val) in img.enumerate_pixels() {
        if *val != blank {
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
