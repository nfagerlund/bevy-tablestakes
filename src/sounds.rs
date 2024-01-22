use bevy::{audio::PlaybackMode, prelude::*};

use crate::{assets_setup::SoundEffects, movement::Landed};

/// Marker struct for audio sink entities that play sound effects. There can be many of these.
#[derive(Component)]
pub struct SfxSink;

/// Thump on landings
pub fn sounds_thumps(
    mut landings: EventReader<Landed>,
    mut commands: Commands,
    sfx: Res<SoundEffects>,
) {
    // Eventually want to locate these in space maybe?? but crawl before u run.
    // I don't care about how many landings happen this frame, so just burn em all at once.
    if landings.read().count() > 0 {
        commands.spawn(AudioSourceBundle {
            source: sfx.thump.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Despawn,
                ..Default::default()
            },
        });
        // wow, hmm, that was easy.
    }
}
