use bevy::prelude::*;

fn main() {
    App::new()
        .add_startup_system(add_people)
        .add_system(hellow_orld)
        .add_system(greet_peeps)
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

// Hmm guess add_startup_system passes it a Commands struct as an argument
fn add_people(mut commands: Commands) {
    commands.spawn().insert(Person).insert(Name("Himbo Wilson".to_string()));
    commands.spawn().insert(Person).insert(Name("Rumpo Bunkus".to_string()));
    commands.spawn().insert(Person).insert(Name("Ryan Malarkey".to_string()));
}

// Whoa WH A T, we're using the type signature to query entities' components???
fn greet_peeps(query: Query<&Name, With<Person>>) {
    for name in query.iter() {
        // dbg!(name);
        println!("low hell, {}!", name.0);
    }
}
