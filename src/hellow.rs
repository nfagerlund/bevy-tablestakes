use bevy::prelude::*;

// Systems are plain functions
fn _hellow_orld() {
    println!("hellow, orld!");
}

#[derive(Component)]
struct Person;

// Oh huh, so name is a component that other kinds of things might have.
#[derive(Component, Debug)]
struct Name(String);

#[derive(Resource)]
struct GreetTimer(Timer); // ??

#[derive(Component, Debug)]
enum Crumbo {
    Gunk(&'static str),
    Zilch,
}

// Hmm guess add_startup_system passes it a Commands struct as an argument
// ^^ update: now we know about SystemParams, you can ask for whatever arguments you want.
fn add_people(mut commands: Commands) {
    commands.spawn((Person, Name("Himbo Wilson".to_string()), Crumbo::Zilch));
    commands.spawn((Person, Name("Rumpo Bunkus".to_string()), Crumbo::Gunk("blub")));
    commands.spawn((Person, Name("Ryan Malarkey".to_string())));
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

fn blunch(
    timer: Res<GreetTimer>,
    query: Query<(&Name, &Crumbo), With<Person>>,
) {
    if timer.0.just_finished() {
        for (name, crumbo) in query.iter() {
            println!("{} has {:?}", name.0, crumbo);
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
            .insert_resource(GreetTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
            .add_startup_system(add_people)
            // .add_system(hellow_orld)
            .add_system(greet_peeps)
            .add_system(blunch.after(greet_peeps));

    }
}
