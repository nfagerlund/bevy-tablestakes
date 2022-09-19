use bevy::asset::{
    AssetIoError, Assets, AssetServer, AssetLoader, AssetPath, BoxedFuture, Handle, LoadContext, LoadedAsset,
};
use bevy::math::prelude::*;
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::{
	render_resource::{Extent3d, TextureDimension, TextureFormat},
	texture::{Image, TextureFormatPixelInfo}
};
use bevy::sprite::{Rect, TextureAtlas, TextureAtlasBuilder};
use bevy::utils::Duration;
use std::collections::HashMap;
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

#[derive(Debug)]
pub struct CharAnimationFrame {
	pub index: usize,
	pub duration: Duration, // ?
	pub origin: Vec2,
	pub walkbox: Option<Rect>,
	// hitbox, hurtbox,
}

pub struct CharAnimationPlugin;

impl Plugin for CharAnimationPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_asset::<CharAnimation>()
			.init_asset_loader::<CharAnimationLoader>()
			.add_startup_system(charanm_test_setup_system);
	}
}

fn charanm_test_setup_system(
	mut commands: Commands,
	asset_server: Res<AssetServer>,
) {
    let test_texture_handle: Handle<Image> = asset_server.load("sprites/sPlayerRun.aseprite");
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
            // DO SOMETHING HERE w/ ?, returning (), and using LoadContext.set_default_asset for its side effects
			let bevy_img = load_aseprite_badly(bytes)?;
			load_context.set_default_asset(LoadedAsset::new(bevy_img));
            Ok(())
        })
	}
}

// Return the real asset, not ()!
// RE-re-reconsidering all this: None of the atlas builders are suitable, they
// all rely on runtime asset collections. So I need to construct a plain sprite
// sheet texture myself, then use TextureAtlas::from_grid_with_padding, because
// it only requires a Handle<Image> (which I can provide).
fn load_aseprite(bytes: &[u8], load_context: &mut LoadContext) -> anyhow::Result<()> {
    let ase = AsepriteFile::read(bytes)?;
	// Assumptions: I'm only using AnimationDirection::Forward, and I'm assuming
	// tag names are unique in the file (which aseprite doesn't guarantee), and
	// I'm assuming there are named tags that cover all the needed animation.
	let width = ase.width();
	let height = ase.height();
	let num_frames = ase.num_frames();

	// Build the texture atlas first!

	// Laziness: I'm slurping every frame into the final asset, so it might
	// include some garbage frames that aren't in any tags. Later, this lets me
	// index into the texture atlas using plain frame IDs without needing to
	// hold onto any translation layer.
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
	// atlas time!!! hardcoding many things
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
	// At this point, the indexing of the atlas's sub-textures matches the
	// original Ase file's frame indices! (although it's usize instead of
	// asefile's u32.) from_grid_with_padding adds grid cells in left-to-right,
	// top-to-bottom order.

	// Now that I know about the indexing ahead of time, I should be able to
	// construct complete animation variants in one go.
	let mut variants: HashMap<String, CharAnimationVariant> = HashMap::new();

	for tag in (0..ase.num_tags()).map(|i| ase.tag(i)) {
		let name = tag.name().to_string();
		let frame_range = tag.from_frame()..=tag.to_frame(); // inclusive
		let frames: Vec<CharAnimationFrame> = frame_range.map(|i| {
			let frame = ase.frame(i);
			let index = i as usize;
			let duration_ms = frame.duration() as u64;
			let duration = Duration::from_millis(duration_ms);

			let walkbox = ase.layer_by_name("walkbox").map(|wb_layer| {
				let wb_image = wb_layer.frame(i).image();
				get_rect_lmao(&wb_image)
			}).flatten();

			let origin = Vec2::ZERO; // TODO idk yet

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

	// OK, now I think we can build the whole enchilada...
	let animation = CharAnimation { variants, texture_atlas: atlas_handle };

	// And, cut!
	load_context.set_default_asset(LoadedAsset::new(animation));
	Ok(())
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
fn get_rect_lmao(img: &RgbaImage) -> Option<Rect> {
    let mut x_min: u32 = u32::MAX;
    let mut x_max: u32 = 0;
    let mut y_min: u32 = u32::MAX;
    let mut y_max: u32 = 0;
	let mut present = false;
    for (x, y, val) in img.enumerate_pixels() {
        if alpha(val) != 0 {
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

// Yoinked+hacked fn from TextureAtlasBuilder (which I can't use directly
// because it relies on runtime asset collections):
// https://github.com/bevyengine/bevy/blob/c27cc59e0d1e305b0ab42b07455c3550ed671740/crates/bevy_sprite/src/texture_atlas_builder.rs#L95
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
