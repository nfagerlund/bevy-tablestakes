use bevy::asset::{
    AssetIoError, AssetLoader, AssetPath, BoxedFuture, Handle, LoadContext, LoadedAsset,
};
use bevy::math::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::texture::Image;
use bevy::sprite::{Rect, TextureAtlas, TextureAtlasBuilder};
use bevy::utils::Duration;
use std::collections::HashMap;
use asefile::AsepriteFile;
// use image;

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
	pub texture_index: usize,
	pub duration: Duration, // ?
	pub walkbox: Option<Rect>,
	// hitbox, hurtbox,
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
fn load_aseprite(bytes: &[u8]) -> anyhow::Result<()> {
    let ase = AsepriteFile::read(bytes)?;
	// Assumptions: I'm only using AnimationDirection::Forward, and I'm assuming
	// tag names are unique in the file (which aseprite doesn't guarantee).
	let atlas_builder = TextureAtlasBuilder::default();


	Ok(())
}

// Test texture creation
fn load_aseprite_badly(bytes: &[u8]) -> anyhow::Result<Image> {
	let ase = AsepriteFile::read(bytes)?;
	let img = ase.frame(0).image();
	let size = Extent3d {
		width: img.width(),
		height: img.height(),
		depth_or_array_layers: 1
	};
	let bevy_img = Image::new(
		size,
		TextureDimension::D2,
		img.into_raw(),
		TextureFormat::Rgba8UnormSrgb,
	);
	dbg!(&bevy_img);
	Ok(bevy_img)
}
