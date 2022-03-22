use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(HelloPlugin)
        .run();
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
