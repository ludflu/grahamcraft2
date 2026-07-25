//! Shared datatypes for the game client.

use bevy::prelude::*;

/// Integer voxel position in the world grid.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BlockCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockCoord {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn as_vec3(self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }
}

/// A block placed or removed by the server.
#[derive(Clone, Copy, Debug)]
pub struct BlockUpdate {
    pub coord: BlockCoord,
    pub block_type: i32,
}

/// Position and identity of a connected player.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RemotePlayerState {
    pub player_id: String,
    pub player_name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Another player appeared in the world.
#[derive(Clone, Debug)]
pub struct PlayerJoin {
    pub state: RemotePlayerState,
}

/// Another player changed position.
#[derive(Clone, Debug)]
pub struct PlayerMove {
    pub player_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Another player disconnected.
#[derive(Clone, Debug)]
pub struct PlayerLeave {
    pub player_id: String,
}

/// Marker on rendered voxel cubes.
#[derive(Component, Clone, Copy, Debug)]
pub struct Voxel {
    pub coord: BlockCoord,
}

/// Marker on local player root entity.
#[derive(Component)]
pub struct LocalPlayer;

/// Marker on the camera pivot used for mouse look.
#[derive(Component)]
pub struct CameraPivot;

/// Marker on remote player avatar roots.
#[derive(Component)]
#[allow(dead_code)]
pub struct RemoteAvatar {
    pub player_id: String,
}

/// Marker on avatar body parts that should be ignored by raycasts.
#[derive(Component)]
pub struct RaycastIgnore;

/// Ten palette colors matching the Python Ursina client.
pub const BLOCK_PALETTE: [Color; 10] = [
    Color::srgb(1.0, 0.0, 0.0),       // red
    Color::srgb(1.0, 0.5, 0.0),       // orange
    Color::srgb(1.0, 1.0, 0.0),       // yellow
    Color::srgb(0.5, 1.0, 0.0),       // lime
    Color::srgb(0.0, 1.0, 0.0),       // green
    Color::srgb(0.25, 0.88, 0.82),    // turquoise
    Color::srgb(0.0, 0.5, 1.0),       // azure
    Color::srgb(0.0, 0.0, 1.0),       // blue
    Color::srgb(0.5, 0.0, 1.0),       // violet
    Color::srgb(1.0, 0.0, 1.0),       // magenta
];

pub fn block_color(block_type: i32) -> Color {
    let index = ((block_type - 1).rem_euclid(BLOCK_PALETTE.len() as i32)) as usize;
    BLOCK_PALETTE[index]
}
