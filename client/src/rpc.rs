//! gRPC client that talks to the game server in a background thread.

use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use tonic::transport::Channel;
use tonic::Request;

use crate::models::{BlockCoord, BlockUpdate, PlayerJoin, PlayerLeave, PlayerMove, RemotePlayerState};

pub mod proto {
    tonic::include_proto!("game");
}

use proto::game_service_client::GameServiceClient;
use proto::{
    BreakBlockRequest, JoinRequest, SetBlockRequest, SubscribeRequest, UpdatePositionRequest,
};

pub const DEFAULT_SERVER: &str = "localhost:50051";
const DEFAULT_BLOCK_TYPE: i32 = 1;
const PLAYER_NAME: &str = "player";
const REQUEST_POLL: Duration = Duration::from_millis(50);

enum BlockAction {
    Place(BlockCoord),
    Break(BlockCoord),
}

pub enum PlayerEvent {
    Join(PlayerJoin),
    Move(PlayerMove),
    Leave(PlayerLeave),
}

#[derive(Clone, Debug)]
pub struct SessionReady {
    pub player_id: String,
    pub spawn_position: (f32, f32, f32),
    pub connection_error: String,
}

/// Handles spawned from the main thread; communicates over channels.
pub struct GameSession {
    action_tx: Sender<BlockAction>,
    position_tx: Sender<(f32, f32, f32)>,
    block_rx: Receiver<BlockUpdate>,
    player_rx: Receiver<PlayerEvent>,
    ready_rx: Receiver<SessionReady>,
}

impl GameSession {
    pub fn start(server: String) -> Self {
        let (block_tx, block_rx) = crossbeam_channel::unbounded();
        let (player_tx, player_rx) = crossbeam_channel::unbounded();
        let (action_tx, action_rx) = crossbeam_channel::unbounded();
        let (position_tx, position_rx) = crossbeam_channel::unbounded();
        let (ready_tx, ready_rx) = crossbeam_channel::bounded(1);

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(run_session(
                server,
                block_tx,
                player_tx,
                action_rx,
                position_rx,
                ready_tx,
            ));
        });

        Self {
            action_tx,
            position_tx,
            block_rx,
            player_rx,
            ready_rx,
        }
    }

    pub fn wait_until_ready(&self, timeout: Duration) -> Option<SessionReady> {
        self.ready_rx.recv_timeout(timeout).ok()
    }

    pub fn place_block(&self, coord: BlockCoord) {
        let _ = self.action_tx.send(BlockAction::Place(coord));
    }

    pub fn break_block(&self, coord: BlockCoord) {
        let _ = self.action_tx.send(BlockAction::Break(coord));
    }

    pub fn send_position(&self, x: f32, y: f32, z: f32) {
        let _ = self.position_tx.send((x, y, z));
    }

    pub fn try_recv_block(&self) -> Option<BlockUpdate> {
        self.block_rx.try_recv().ok()
    }

    pub fn try_recv_player(&self) -> Result<PlayerEvent, TryRecvError> {
        self.player_rx.try_recv()
    }
}

async fn run_session(
    server: String,
    block_tx: Sender<BlockUpdate>,
    player_tx: Sender<PlayerEvent>,
    action_rx: Receiver<BlockAction>,
    position_rx: Receiver<(f32, f32, f32)>,
    ready_tx: Sender<SessionReady>,
) {
    if let Err(error) = connect_and_run(
        &server,
        &block_tx,
        &player_tx,
        &action_rx,
        &position_rx,
        &ready_tx,
    )
    .await
    {
        let _ = ready_tx.send(SessionReady {
            player_id: String::new(),
            spawn_position: (0.0, 0.0, 0.0),
            connection_error: error,
        });
    }
}

async fn connect_and_run(
    server: &str,
    block_tx: &Sender<BlockUpdate>,
    player_tx: &Sender<PlayerEvent>,
    action_rx: &Receiver<BlockAction>,
    position_rx: &Receiver<(f32, f32, f32)>,
    ready_tx: &Sender<SessionReady>,
) -> Result<(), String> {
    let channel = Channel::from_shared(normalize_server_address(server))
        .map_err(|err| format!("Invalid server address {server}: {err}"))?
        .connect()
        .await
        .map_err(|_| {
            format!(
                "Could not reach game server at {server}. \
                 Check that the server is running and the IP is correct."
            )
        })?;

    let mut client = GameServiceClient::new(channel);

    let join_response = client
        .join(Request::new(JoinRequest {
            player_name: PLAYER_NAME.to_string(),
        }))
        .await
        .map_err(|err| format!("gRPC error from {server}: {} {}", err.code(), err.message()))?
        .into_inner();

    let player_id = join_response.player_id.clone();
    let mut spawn_position = (0.0, 0.0, 0.0);

    for player in &join_response.players {
        if player.player_id == player_id {
            spawn_position = (player.x, player.y, player.z);
        } else {
            enqueue_join(player_tx, player);
        }
    }

    if let Some(world) = join_response.world {
        for block in world.blocks {
            enqueue_block(block_tx, block.x, block.y, block.z, block.block_type);
        }
    }

    let _ = ready_tx.send(SessionReady {
        player_id: player_id.clone(),
        spawn_position,
        connection_error: String::new(),
    });

    let events = client
        .subscribe_events(Request::new(SubscribeRequest {
            player_id: player_id.clone(),
        }))
        .await
        .map_err(|err| format!("gRPC subscribe error: {} {}", err.code(), err.message()))?
        .into_inner();

    let player_tx_events = player_tx.clone();
    let block_tx_events = block_tx.clone();
    tokio::spawn(async move {
        let mut events = events;
        while let Ok(Some(event)) = events.message().await {
            dispatch_event(&player_tx_events, &block_tx_events, &event);
        }
    });

    loop {
        while let Ok(action) = action_rx.try_recv() {
            match action {
                BlockAction::Place(coord) => {
                    let _ = client
                        .set_block(Request::new(SetBlockRequest {
                            player_id: player_id.clone(),
                            x: coord.x,
                            y: coord.y,
                            z: coord.z,
                            block_type: DEFAULT_BLOCK_TYPE,
                        }))
                        .await;
                }
                BlockAction::Break(coord) => {
                    let _ = client
                        .break_block(Request::new(BreakBlockRequest {
                            player_id: player_id.clone(),
                            x: coord.x,
                            y: coord.y,
                            z: coord.z,
                        }))
                        .await;
                }
            }
        }

        let mut latest_position = None;
        while let Ok(pos) = position_rx.try_recv() {
            latest_position = Some(pos);
        }
        if let Some((x, y, z)) = latest_position {
            let _ = client
                .update_position(Request::new(UpdatePositionRequest {
                    player_id: player_id.clone(),
                    x,
                    y,
                    z,
                }))
                .await;
        }

        tokio::time::sleep(REQUEST_POLL).await;
    }
}

fn dispatch_event(
    player_tx: &Sender<PlayerEvent>,
    block_tx: &Sender<BlockUpdate>,
    event: &proto::GameEvent,
) {
    use proto::game_event::Event;

    match &event.event {
        Some(Event::BlockChange(change)) => {
            enqueue_block(block_tx, change.x, change.y, change.z, change.block_type);
        }
        Some(Event::PlayerMove(mv)) => {
            let _ = player_tx.send(PlayerEvent::Move(PlayerMove {
                player_id: mv.player_id.clone(),
                x: mv.x,
                y: mv.y,
                z: mv.z,
            }));
        }
        Some(Event::PlayerJoin(join)) => {
            if let Some(player) = &join.player {
                enqueue_join(player_tx, player);
            }
        }
        Some(Event::PlayerLeave(leave)) => {
            let _ = player_tx.send(PlayerEvent::Leave(PlayerLeave {
                player_id: leave.player_id.clone(),
            }));
        }
        None => {}
    }
}

fn enqueue_block(block_tx: &Sender<BlockUpdate>, x: i32, y: i32, z: i32, block_type: i32) {
    let _ = block_tx.send(BlockUpdate {
        coord: BlockCoord::new(x, y, z),
        block_type,
    });
}

fn enqueue_join(player_tx: &Sender<PlayerEvent>, player: &proto::PlayerState) {
    let _ = player_tx.send(PlayerEvent::Join(PlayerJoin {
        state: RemotePlayerState {
            player_id: player.player_id.clone(),
            player_name: player.player_name.clone(),
            x: player.x,
            y: player.y,
            z: player.z,
        },
    }));
}

pub fn normalize_server_address(server: &str) -> String {
    if server.starts_with("http://") || server.starts_with("https://") {
        return server.to_string();
    }
    if server.contains(':') {
        return format!("http://{server}");
    }
    format!("http://{server}:50051")
}

#[cfg(test)]
mod tests {
    use super::normalize_server_address;

    #[test]
    fn normalize_adds_scheme_and_default_port() {
        assert_eq!(
            normalize_server_address("localhost"),
            "http://localhost:50051"
        );
        assert_eq!(
            normalize_server_address("192.168.1.10"),
            "http://192.168.1.10:50051"
        );
        assert_eq!(
            normalize_server_address("192.168.1.10:50051"),
            "http://192.168.1.10:50051"
        );
        assert_eq!(
            normalize_server_address("http://example.com:50051"),
            "http://example.com:50051"
        );
    }
}
