use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tonic::{Request, Response, Status};

use crate::state::{GameEvent, GameState, Player};

pub mod proto {
    tonic::include_proto!("game");
}

use proto::game_service_server::{GameService, GameServiceServer};
use proto::{
    BlockChange, BlockEntry, BreakBlockRequest, BreakBlockResponse, GameEvent as ProtoGameEvent,
    JoinRequest, JoinResponse, PlayerJoin, PlayerLeave, PlayerMove, PlayerState, SetBlockRequest,
    SetBlockResponse, SubscribeRequest, UpdatePositionRequest, UpdatePositionResponse,
    WorldSnapshot,
};

pub struct GameServiceImpl {
    state: Arc<RwLock<GameState>>,
}

impl GameServiceImpl {
    pub fn new(state: Arc<RwLock<GameState>>) -> Self {
        Self { state }
    }

    pub fn into_server(self) -> GameServiceServer<Self> {
        GameServiceServer::new(self)
    }

    fn player_to_proto(player: &Player) -> PlayerState {
        PlayerState {
            player_id: player.id.clone(),
            player_name: player.name.clone(),
            x: player.x,
            y: player.y,
            z: player.z,
        }
    }

    fn event_to_proto(event: GameEvent) -> ProtoGameEvent {
        match event {
            GameEvent::BlockChange {
                x,
                y,
                z,
                block_type,
            } => ProtoGameEvent {
                event: Some(proto::game_event::Event::BlockChange(BlockChange {
                    x,
                    y,
                    z,
                    block_type,
                })),
            },
            GameEvent::PlayerMove { player_id, x, y, z } => ProtoGameEvent {
                event: Some(proto::game_event::Event::PlayerMove(PlayerMove {
                    player_id,
                    x,
                    y,
                    z,
                })),
            },
            GameEvent::PlayerJoin(player) => ProtoGameEvent {
                event: Some(proto::game_event::Event::PlayerJoin(PlayerJoin {
                    player: Some(Self::player_to_proto(&player)),
                })),
            },
            GameEvent::PlayerLeave { player_id } => ProtoGameEvent {
                event: Some(proto::game_event::Event::PlayerLeave(PlayerLeave {
                    player_id,
                })),
            },
        }
    }
}

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        let name = request.into_inner().player_name;
        let player = GameState::join(&self.state, name).await;
        let guard = self.state.read().await;
        let players: Vec<PlayerState> = guard.players.values().map(Self::player_to_proto).collect();
        let world = WorldSnapshot {
            size_x: guard.world.size_x(),
            size_y: guard.world.size_y(),
            size_z: guard.world.size_z(),
            blocks: guard
                .world
                .blocks()
                .into_iter()
                .map(|block| BlockEntry {
                    x: block.x,
                    y: block.y,
                    z: block.z,
                    block_type: block.block_type,
                })
                .collect(),
        };
        Ok(Response::new(JoinResponse {
            player_id: player.id,
            world: Some(world),
            players,
        }))
    }

    async fn set_block(
        &self,
        request: Request<SetBlockRequest>,
    ) -> Result<Response<SetBlockResponse>, Status> {
        let req = request.into_inner();
        let success = GameState::set_block(&self.state, req.x, req.y, req.z, req.block_type).await;
        Ok(Response::new(SetBlockResponse { success }))
    }

    async fn break_block(
        &self,
        request: Request<BreakBlockRequest>,
    ) -> Result<Response<BreakBlockResponse>, Status> {
        let req = request.into_inner();
        let success = GameState::break_block(&self.state, req.x, req.y, req.z).await;
        Ok(Response::new(BreakBlockResponse { success }))
    }

    async fn update_position(
        &self,
        request: Request<UpdatePositionRequest>,
    ) -> Result<Response<UpdatePositionResponse>, Status> {
        let req = request.into_inner();
        let success =
            GameState::update_position(&self.state, &req.player_id, req.x, req.y, req.z).await;
        Ok(Response::new(UpdatePositionResponse { success }))
    }

    type SubscribeEventsStream = Pin<Box<dyn Stream<Item = Result<ProtoGameEvent, Status>> + Send>>;

    async fn subscribe_events(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        let receiver = {
            let guard = self.state.read().await;
            guard.subscribe()
        };
        let stream = BroadcastStream::new(receiver)
            .filter_map(|result| result.ok())
            .map(|event| Ok(Self::event_to_proto(event)));
        Ok(Response::new(Box::pin(stream)))
    }
}

#[cfg(test)]
mod tests {
    use super::proto::game_event::Event;
    use super::*;
    use crate::state::GameState;
    use crate::world::{FLOOR_SIZE, WORLD_SIZE_X, WORLD_SIZE_Y, WORLD_SIZE_Z};
    use tokio_stream::StreamExt;
    use tonic::Request;

    #[tokio::test]
    async fn join_returns_player_id_and_world_bounds() {
        let service = GameServiceImpl::new(GameState::new());
        let response = service
            .join(Request::new(JoinRequest {
                player_name: "bob".into(),
            }))
            .await
            .expect("join should succeed")
            .into_inner();

        assert!(!response.player_id.is_empty());
        assert_eq!(response.players.len(), 1);
        let world = response.world.expect("world snapshot should be present");
        assert_eq!(world.size_x, WORLD_SIZE_X);
        assert_eq!(world.size_y, WORLD_SIZE_Y);
        assert_eq!(world.size_z, WORLD_SIZE_Z);
        assert_eq!(world.blocks.len(), (FLOOR_SIZE * FLOOR_SIZE) as usize);
    }

    #[tokio::test]
    async fn set_block_rpc_updates_world() {
        let state = GameState::new();
        let service = GameServiceImpl::new(state.clone());
        let player = GameState::join(&state, "builder".into()).await;

        let response = service
            .set_block(Request::new(SetBlockRequest {
                player_id: player.id,
                x: 3,
                y: 4,
                z: 5,
                block_type: 8,
            }))
            .await
            .expect("set_block should succeed")
            .into_inner();

        assert!(response.success);
        assert_eq!(state.read().await.world.get_block(3, 4, 5), Some(8));
    }

    #[tokio::test]
    async fn break_block_rpc_removes_block() {
        let state = GameState::new();
        let service = GameServiceImpl::new(state.clone());
        let player = GameState::join(&state, "miner".into()).await;
        GameState::set_block(&state, 1, 1, 1, 6).await;

        let response = service
            .break_block(Request::new(BreakBlockRequest {
                player_id: player.id,
                x: 1,
                y: 1,
                z: 1,
            }))
            .await
            .expect("break_block should succeed")
            .into_inner();

        assert!(response.success);
        assert_eq!(state.read().await.world.get_block(1, 1, 1), Some(0));
    }

    #[tokio::test]
    async fn update_position_rpc_moves_player() {
        let state = GameState::new();
        let service = GameServiceImpl::new(state.clone());
        let player = GameState::join(&state, "walker".into()).await;

        let response = service
            .update_position(Request::new(UpdatePositionRequest {
                player_id: player.id.clone(),
                x: 10.0,
                y: 20.0,
                z: 30.0,
            }))
            .await
            .expect("update_position should succeed")
            .into_inner();

        assert!(response.success);
        let stored = state.read().await.players.get(&player.id).cloned().unwrap();
        assert!((stored.x, stored.y, stored.z) == (10.0, 20.0, 30.0));
    }

    #[tokio::test]
    async fn subscribe_events_streams_block_changes() {
        let state = GameState::new();
        let service = GameServiceImpl::new(state.clone());
        let mut stream = service
            .subscribe_events(Request::new(SubscribeRequest {
                player_id: "listener".into(),
            }))
            .await
            .expect("subscribe should succeed")
            .into_inner();

        GameState::set_block(&state, 7, 8, 9, 3).await;

        let event = stream
            .next()
            .await
            .expect("stream should yield an event")
            .expect("event should be ok");

        match event.event {
            Some(Event::BlockChange(change)) => {
                assert_eq!(
                    (change.x, change.y, change.z, change.block_type),
                    (7, 8, 9, 3)
                );
            }
            other => panic!("unexpected proto event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn join_includes_existing_blocks_in_snapshot() {
        let state = GameState::new();
        GameState::set_block(&state, 2, 2, 2, 4).await;
        let service = GameServiceImpl::new(state);

        let world = service
            .join(Request::new(JoinRequest {
                player_name: "latecomer".into(),
            }))
            .await
            .expect("join should succeed")
            .into_inner()
            .world
            .expect("world snapshot should be present");

        assert_eq!(world.blocks.len(), (FLOOR_SIZE * FLOOR_SIZE + 1) as usize);
        let placed = world
            .blocks
            .iter()
            .find(|block| block.x == 2 && block.y == 2 && block.z == 2)
            .expect("placed block should be in snapshot");
        assert_eq!(placed.block_type, 4);
    }
}
