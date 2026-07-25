mod app;
mod aiming;
mod models;
mod player;
mod player_model;
mod remote_players;
mod rpc;
mod state;
mod ui;
mod voxels;

use std::time::Duration;

use app::GamePlugin;
use bevy::prelude::*;
use clap::Parser;
use rpc::GameSession;
use state::ClientState;

#[derive(Parser, Debug)]
#[command(name = "grahamcraft-client", about = "Grahamcraft multiplayer client")]
struct Args {
    /// Game server address as host or host:port
    #[arg(short, long, default_value = rpc::DEFAULT_SERVER)]
    server: String,
}

fn main() {
    let args = Args::parse();
    let server = rpc::normalize_server_address(&args.server);
    let session = GameSession::start(server.clone());

    let ready = match session.wait_until_ready(Duration::from_secs(10)) {
        Some(ready) => ready,
        None => {
            eprintln!("Failed to connect to the game server.");
            eprintln!("Timed out after 10s waiting for {server}");
            return;
        }
    };

    if !ready.connection_error.is_empty() {
        eprintln!("Failed to connect to the game server.");
        eprintln!("{}", ready.connection_error);
        return;
    }

    let mut pending_blocks = Vec::new();
    while let Some(update) = session.try_recv_block() {
        pending_blocks.push(update);
    }

    let client_state = ClientState::new(session, ready.player_id, ready.spawn_position);

    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Grahamcraft".into(),
                    ..default()
                }),
                ..default()
            }),
        )
        .insert_resource(client_state)
        .insert_resource(voxels::PendingBlocks(pending_blocks))
        .add_plugins(GamePlugin)
        .run();
}
