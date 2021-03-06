use bevy::prelude::*;
use crate::{
    ActiveGamepad,
};

// Input time!

/// helper function: forward the axes resource (and a gamepad id) to it, get a vec back.
/// Note: `gilrs`, Bevy's gamepad library, only supports Xinput on windows. boo.
pub fn get_gamepad_movement_vector(gamepad: Gamepad, axes: Res<Axis<GamepadAxis>>) -> Option<Vec2> {
    let x_axis = GamepadAxis(gamepad, GamepadAxisType::LeftStickX);
    let y_axis = GamepadAxis(gamepad, GamepadAxisType::LeftStickY);
    let x = axes.get(x_axis)?;
    let y = axes.get(y_axis)?;
    Some(Vec2::new(x, y))
}

/// helper function: forward keycodes to it, get a vec back.
pub fn get_kb_movement_vector(keys: Res<Input<KeyCode>>) -> Vec2 {
    let mut x = 0f32;
    let mut y = 0f32;
    if keys.pressed(KeyCode::Left) {
        x -= 1.0;
    }
    if keys.pressed(KeyCode::Right) {
        x += 1.0;
    }
    if keys.pressed(KeyCode::Up) {
        y += 1.0; // bc, opposite of other engines so far
    }
    if keys.pressed(KeyCode::Down) {
        y -= 1.0;
    }
    Vec2::new(x, y)
}

/// System for noticing when gamepads are added/removed and marking which
/// gamepad is active for the player.
pub fn connect_gamepads_system(
    mut commands: Commands,
    active_gamepad: Option<Res<ActiveGamepad>>,
    mut gamepad_events: EventReader<GamepadEvent>,
    // ^^ eventreader params have to be mutable because reading events immutably
    // still updates an internal tracking cursor on the reader instance. cool.
) {
    for GamepadEvent(id, kind) in gamepad_events.iter() {
        match kind {
            GamepadEventType::Connected => {
                info!("pad up: {:?}", id);
                // let's see, I de-focused the cookbook tab, so what do *I* want to have happen?
                // First pad in gets it, but if another pad hits start, it'll take over. Nice.
                if active_gamepad.is_none() {
                    commands.insert_resource(ActiveGamepad(*id));
                }
            },
            GamepadEventType::Disconnected => {
                info!("pad down: {:?}", id);
                // byeeee
                // ok, I'm back to the example code, what's going on here:
                if let Some(ActiveGamepad(old_id)) = active_gamepad.as_deref() {
                    if old_id == id {
                        commands.remove_resource::<ActiveGamepad>();
                        // haven't really had to turbofish before now. zoom zoom glub glub.
                    }
                }
            },
            GamepadEventType::ButtonChanged(GamepadButtonType::Start, val) if *val == 1.0 => {
                info!("Pressed start: {:?}", id);
                // If there's an active gamepad...
                if let Some(ActiveGamepad(old_id)) = active_gamepad.as_deref() {
                    // ...but it's not the one you just pressed start on...
                    if old_id != id {
                        // ...then let it take over.
                        commands.insert_resource(ActiveGamepad(*id));
                        // per the cheatbook: "If you insert a resource of a
                        // type that already exists, it will be overwritten."
                    }
                }
            },
            // ignore other events for now, ^H^H^H (never mind) but fyi see
            // examples/input/gamepad_input_events.rs in bevy. The
            // ButtonChanged(button_type, value) and AxisChanged(axis_type,
            // value) all return new values as floats, I guess so you can handle
            // buttons and analog axes with similar code.
            _ => {},
        }
    }
}

