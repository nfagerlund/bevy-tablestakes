use bevy::prelude::*;

fn main() {
    App::new().add_system(hellow_orld).run();
}

fn hellow_orld() {
    println!("hellow, orld!");
}
