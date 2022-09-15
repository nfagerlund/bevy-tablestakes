use bevy::asset::{
    AssetIoError, AssetLoader, AssetPath, BoxedFuture, Handle, LoadContext, LoadedAsset,
};
use bevy::math::prelude::*;
use bevy::sprite::{Rect, TextureAtlas};
use bevy::utils::Duration;
use std::collections::HashMap;
use asefile::AsepriteFile;
use image;

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

struct CharAnimationLoader;

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
            Ok(())
        })
	}
}

// Return the real asset, not ()!
// fn load_aseprite(bytes: &[u8]) -> anyhow::Result<()> {
//     let ase = AsepriteFile::read(bytes);
// }
