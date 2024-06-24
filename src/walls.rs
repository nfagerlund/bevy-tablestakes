use crate::{
    collision::{centered_rect, Solid, Walkbox},
    phys_space::PhysOffset,
};
use bevy::{math::Vec2, prelude::Bundle};
use bevy_ecs_ldtk::prelude::*;

/// Wall bundle for tilemap walls
#[derive(Bundle)]
pub struct Wall {
    solid: Solid,
    walkbox: Walkbox,
    offset: PhysOffset,
    int_grid_cell: IntGridCell,
    // transform: Transform, // This is needed, but it's handled by the plugin.
}

// Custom impl instead of derive bc... you'll see!
impl LdtkIntCell for Wall {
    fn bundle_int_cell(int_grid_cell: IntGridCell, layer_instance: &LayerInstance) -> Self {
        // there!! v. proud of finding this, the example just cheated w/ prior knowledge.
        let grid_size = layer_instance.grid_size as f32;
        let translation_offset = Vec2::new(
            grid_size / 2.0 + layer_instance.px_total_offset_x as f32,
            grid_size / 2.0 + layer_instance.px_total_offset_y as f32,
        );
        Wall {
            solid: Solid,
            // the plugin puts tile anchor points in the center:
            walkbox: Walkbox(centered_rect(grid_size, grid_size)),
            offset: PhysOffset(translation_offset),
            int_grid_cell,
        }
    }
}
