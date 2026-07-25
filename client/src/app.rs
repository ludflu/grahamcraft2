//! Main game plugin wiring networking, world, and player systems.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::aiming::{handle_block_input, update_crosshair_position};
use crate::player::{
    ensure_on_ground, grab_cursor, player_gravity, player_look, player_move, snap_to_ground,
    spawn_player, sync_position, toggle_gravity,
};
use crate::remote_players::RemotePlayers;
use crate::rpc::PlayerEvent;
use crate::state::ClientState;
use crate::ui::{setup_crosshair, CrosshairSmooth};
use crate::voxels::{setup_voxel_world, PendingBlocks, VoxelWorld};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CrosshairSmooth>()
            .add_systems(
            Startup,
            (
                setup_scene,
                setup_voxel_world,
                setup_crosshair,
                load_initial_world,
                spawn_player,
                initial_snap,
            )
                .chain(),
        )
            .add_systems(
                Update,
                (
                    grab_cursor,
                    drain_network_events,
                    player_look,
                    player_move,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (ensure_on_ground, player_gravity)
                    .chain()
                    .after(player_move),
            )
            .add_systems(
                Update,
                (
                    sync_position,
                    toggle_gravity,
                    update_crosshair_position,
                    handle_block_input,
                    remote_players_tick,
                )
                    .after(player_gravity),
            );
    }
}

fn load_initial_world(
    client: Res<ClientState>,
    mut pending: ResMut<PendingBlocks>,
    mut world: ResMut<VoxelWorld>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for update in pending.0.drain(..) {
        world.apply(&mut commands, &mut materials, update);
    }

    while let Some(update) = client.session.try_recv_block() {
        world.apply(&mut commands, &mut materials, update);
    }

    info!("Loaded {} blocks", world.block_count());
    let spawn = client.spawn_position;
    if !world.has_support_at(spawn.x, spawn.z) {
        warn!(
            "No floor blocks under spawn ({:.1}, {:.1}); using default floor height",
            spawn.x, spawn.z
        );
    }
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.5, 0.0)),
    ));
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
    });
    commands.insert_resource(ClearColor(Color::srgb(0.529, 0.808, 0.922)));
    commands.insert_resource(RemotePlayers::default());
}

fn initial_snap(
    mut client: ResMut<ClientState>,
    mut player: Query<&mut Transform, With<crate::models::LocalPlayer>>,
    world: Res<VoxelWorld>,
) {
    if client.snapped {
        return;
    }
    let Ok(mut transform) = player.get_single_mut() else {
        return;
    };
    transform.translation = client.spawn_position;
    snap_to_ground(&mut transform, &mut client, &world);
    client.last_position = transform.translation;
    client.session.send_position(
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
    );
    client.snapped = true;
}

#[allow(clippy::too_many_arguments)]
fn drain_network_events(
    client: Res<ClientState>,
    mut world: ResMut<VoxelWorld>,
    mut remote: ResMut<RemotePlayers>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<crate::models::LocalPlayer>>,
) {
    let cube_mesh = world.cube_mesh();

    while let Some(update) = client.session.try_recv_block() {
        world.apply(&mut commands, &mut materials, update);
    }

    let mut pending_moves = HashMap::new();
    loop {
        match client.session.try_recv_player() {
            Ok(PlayerEvent::Join(join)) => {
                let notify_server = join.state.player_id != client.player_id;
                remote.apply_join(
                    &mut commands,
                    cube_mesh.clone(),
                    &mut materials,
                    join,
                    &world,
                );
                if notify_server {
                    if let Ok(transform) = player.get_single() {
                        let pos = transform.translation;
                        client.session.send_position(pos.x, pos.y, pos.z);
                    }
                }
            }
            Ok(PlayerEvent::Move(mv)) => {
                pending_moves.insert(mv.player_id.clone(), mv);
            }
            Ok(PlayerEvent::Leave(leave)) => {
                remote.apply_leave(&mut commands, leave);
            }
            Err(_) => break,
        }
    }

    for mv in pending_moves.into_values() {
        remote.apply_move_maybe_spawn(
            &mut commands,
            cube_mesh.clone(),
            &mut materials,
            mv,
            &world,
        );
    }
}

fn remote_players_tick(
    time: Res<Time>,
    mut remote: ResMut<RemotePlayers>,
    transforms: Query<&mut Transform, With<crate::models::RemoteAvatar>>,
) {
    remote.tick(time, transforms);
}
