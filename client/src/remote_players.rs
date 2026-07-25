//! Render other connected players from server events.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::models::{PlayerJoin, PlayerLeave, PlayerMove, RemoteAvatar};
use crate::player_model::spawn_remote_avatar;
use crate::voxels::VoxelWorld;

const FLOOR_TOP_Y: f32 = 1.0;
const LERP_SPEED: f32 = 14.0;

#[derive(Resource, Default)]
pub struct RemotePlayers {
    local_player_id: String,
    figures: HashMap<String, Entity>,
    targets: HashMap<String, Vec3>,
}

impl RemotePlayers {
    pub fn set_local_player(&mut self, player_id: String) {
        self.local_player_id = player_id;
    }

    pub fn apply_join(
        &mut self,
        commands: &mut Commands,
        cube_mesh: Handle<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        event: PlayerJoin,
        world: &VoxelWorld,
    ) {
        let state = event.state;
        if state.player_id == self.local_player_id {
            return;
        }
        self.spawn_avatar(
            commands,
            cube_mesh,
            materials,
            &state.player_id,
            Vec3::new(state.x, state.y, state.z),
            true,
            world,
        );
    }

    pub fn apply_move_maybe_spawn(
        &mut self,
        commands: &mut Commands,
        cube_mesh: Handle<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        event: PlayerMove,
        world: &VoxelWorld,
    ) {
        if event.player_id == self.local_player_id {
            return;
        }
        self.targets
            .insert(event.player_id.clone(), Vec3::new(event.x, event.y, event.z));
        if !self.figures.contains_key(&event.player_id) {
            self.spawn_avatar(
                commands,
                cube_mesh,
                materials,
                &event.player_id,
                Vec3::new(event.x, event.y, event.z),
                false,
                world,
            );
        }
    }

    pub fn apply_leave(&mut self, commands: &mut Commands, event: PlayerLeave) {
        self.targets.remove(&event.player_id);
        if let Some(entity) = self.figures.remove(&event.player_id) {
            commands.entity(entity).despawn();
        }
    }

    pub fn tick(&mut self, time: Res<Time>, mut transforms: Query<&mut Transform, With<RemoteAvatar>>) {
        let step = (time.delta_secs() * LERP_SPEED).min(1.0);
        for (player_id, entity) in &self.figures {
            let Some(target) = self.targets.get(player_id) else {
                continue;
            };
            if let Ok(mut transform) = transforms.get_mut(*entity) {
                transform.translation = transform.translation.lerp(*target, step);
            }
        }
    }

    fn spawn_avatar(
        &mut self,
        commands: &mut Commands,
        cube_mesh: Handle<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        player_id: &str,
        server_pos: Vec3,
        snap_to_ground: bool,
        world: &VoxelWorld,
    ) {
        if let Some(entity) = self.figures.remove(player_id) {
            commands.entity(entity).despawn();
        }

        let feet_y = if snap_to_ground {
            world
                .ground_y_at(server_pos.x, server_pos.y, server_pos.z)
                .unwrap_or(FLOOR_TOP_Y)
        } else {
            server_pos.y
        };
        let target = Vec3::new(server_pos.x, feet_y, server_pos.z);
        self.targets.insert(player_id.to_string(), target);

        let root = spawn_remote_avatar(commands, cube_mesh, materials, player_id, target);
        commands.entity(root).insert(RemoteAvatar {
            player_id: player_id.to_string(),
        });
        self.figures.insert(player_id.to_string(), root);
    }
}
