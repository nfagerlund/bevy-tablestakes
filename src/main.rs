use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(HelloPlugin)
        .add_startup_system(setup_sprites)
        .add_system(animate_sprites_system)
        .run();
}

fn animate_sprites_system(
    time: Res<Time>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    mut query: Query<(&mut Timer, &mut TextureAtlasSprite, &Handle<TextureAtlas>)>,
    // ^^ ok, the timer I added myself, and the latter two were part of the bundle.
) {
    for (mut timer, mut sprite, texture_atlas_handle) in query.iter_mut() {
        timer.tick(time.delta()); // ok, I remember you. advance the timer.
        if timer.finished() {
            let texture_atlas = texture_atlases.get(texture_atlas_handle).unwrap(); // uh ok. btw, how do we avoid the unwraps in this runtime?
            sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
            // ^^ Ah. OK. We're doing some realll basic flipbooking here. But also, note that the TextureAtlasSprite struct ONLY has color/index/flip_(x|y)/custom_size props, it's meant to always be paired with a textureatlas handle and it doesn't hold its own reference to one. ECS lifestyles.
        }
    }
}

fn setup_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    // Time to start re-typing everything from bevy/examples/2d/sprite_sheet.rs. well, we all start somewhere.

    // vv OK, so apparently asset_server.load() CAN infer the type of a handle for a receiving
    // binding without a type annotation, but only by looking *ahead* at where you consume the
    // handle! That's some rust magic. Anyway, in my case I'm still exploring so I guess I'll just
    // annotate.
    let texture_handle: Handle<Image> = asset_server.load("sprites/sPlayerRun_strip32.png");
    // vv AH ha, and here's the bit I would want some automation for. Should be easy lol.
    let texture_atlas = TextureAtlas::from_grid(texture_handle, Vec2::new(17.0, 24.0), 32, 1);
    let texture_atlas_handle = texture_atlases.add(texture_atlas);

    commands.spawn_bundle(OrthographicCameraBundle::new_2d()); // Oh, hmm, gonna want to move that to another system later.
    commands
        .spawn_bundle(SpriteSheetBundle {
            texture_atlas: texture_atlas_handle,
            transform: Transform::from_scale(Vec3::splat(3.0)),
            ..Default::default()
        })
        .insert(Timer::from_seconds(0.1, true)); // <- oh, no, ok, gotcha, that's adding a component on the spawned entity from that bundle.
}

// Systems are plain functions
fn hellow_orld() {
    println!("hellow, orld!");
}

#[derive(Component)]
struct Person;

// Oh huh, so name is a component that other kinds of things might have.
#[derive(Component, Debug)]
struct Name(String);

struct GreetTimer(Timer); // ??

// Hmm guess add_startup_system passes it a Commands struct as an argument
fn add_people(mut commands: Commands) {
    commands.spawn().insert(Person).insert(Name("Himbo Wilson".to_string()));
    commands.spawn().insert(Person).insert(Name("Rumpo Bunkus".to_string()));
    commands.spawn().insert(Person).insert(Name("Ryan Malarkey".to_string()));
}

// Whoa WH A T, we're using the type signature to query entities' components???
fn greet_peeps(
    time: Res<Time>,
    mut timer: ResMut<GreetTimer>,
    query: Query<&Name, With<Person>>,
) {
    // Update our timer with time elapsed since last tick; if that ends the
    // timer, squawk.
    if timer.0.tick(time.delta()).just_finished() {
        for name in query.iter() {
            // dbg!(name);
            println!("low hell, {}!", name.0);
        }
    }
}

pub struct HelloPlugin;
impl Plugin for HelloPlugin {
    fn build(&self, app: &mut App) {
        // add app stuff
        // per example code:
            // the reason we call from_seconds with the true flag is to make the timer repeat itself
        // ...oh. you're just explaining the fact that the second arg to from_seconds is named `repeating`. ok.
        app
            .insert_resource(GreetTimer(Timer::from_seconds(2.0, true)))
            .add_startup_system(add_people)
            // .add_system(hellow_orld)
            .add_system(greet_peeps);

    }
}
