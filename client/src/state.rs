//! Shared client state for networking and local player physics.

use std::time::Instant;

use bevy::prelude::*;

use crate::rpc::GameSession;

#[derive(Resource)]
pub struct ClientState {
    pub session: GameSession,
    pub player_id: String,
    pub spawn_position: Vec3,
    pub gravity_enabled: bool,
    pub grounded: bool,
    pub vertical_velocity: f32,
    pub last_position: Vec3,
    pub last_sync: Instant,
    pub was_airborne: bool,
    pub snapped: bool,
}

impl ClientState {
    pub fn new(session: GameSession, player_id: String, spawn: (f32, f32, f32)) -> Self {
        let spawn_position = Vec3::new(spawn.0, spawn.1, spawn.2);
        Self {
            session,
            player_id,
            spawn_position,
            gravity_enabled: true,
            grounded: false,
            vertical_velocity: 0.0,
            last_position: spawn_position,
            last_sync: Instant::now(),
            was_airborne: false,
            snapped: false,
        }
    }
}
