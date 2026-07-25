use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::world::World;

const EVENT_CHANNEL_CAPACITY: usize = 256;

/// A connected player's position and display name.
#[derive(Clone, Debug)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Events broadcast to all subscribed clients.
#[derive(Clone, Debug)]
pub enum GameEvent {
    BlockChange {
        x: i32,
        y: i32,
        z: i32,
        block_type: i32,
    },
    PlayerMove {
        player_id: String,
        x: f32,
        y: f32,
        z: f32,
    },
    PlayerJoin(Player),
    #[allow(dead_code)]
    PlayerLeave {
        player_id: String,
    },
}

/// Shared game state: world, players, and event bus.
pub struct GameState {
    pub world: World,
    pub players: HashMap<String, Player>,
    events: broadcast::Sender<GameEvent>,
}

impl GameState {
    pub fn new() -> Arc<RwLock<Self>> {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Arc::new(RwLock::new(Self {
            world: World::create_initial(),
            players: HashMap::new(),
            events,
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GameEvent> {
        self.events.subscribe()
    }

    fn broadcast(&self, event: GameEvent) {
        let _ = self.events.send(event);
    }

    fn spawn_offset(id: &Uuid) -> (f32, f32) {
        let bytes = id.as_bytes();
        let offset_x = (bytes[0] % 7) as f32 - 3.0;
        let offset_z = (bytes[1] % 7) as f32 - 3.0;
        (offset_x, offset_z)
    }

    pub async fn join(state: &Arc<RwLock<Self>>, name: String) -> Player {
        let id = Uuid::new_v4();
        let (base_x, base_y, base_z) = World::spawn_position();
        let (offset_x, offset_z) = Self::spawn_offset(&id);
        let player = Player {
            id: id.to_string(),
            name,
            x: base_x + offset_x,
            y: base_y,
            z: base_z + offset_z,
        };
        let join_event = GameEvent::PlayerJoin(player.clone());
        let mut guard = state.write().await;
        guard.players.insert(player.id.clone(), player.clone());
        guard.broadcast(join_event);
        player
    }

    pub async fn set_block(
        state: &Arc<RwLock<Self>>,
        x: i32,
        y: i32,
        z: i32,
        block_type: i32,
    ) -> bool {
        let mut guard = state.write().await;
        if !guard.world.set_block(x, y, z, block_type) {
            return false;
        }
        guard.broadcast(GameEvent::BlockChange {
            x,
            y,
            z,
            block_type,
        });
        true
    }

    pub async fn break_block(state: &Arc<RwLock<Self>>, x: i32, y: i32, z: i32) -> bool {
        let mut guard = state.write().await;
        if !guard.world.break_block(x, y, z) {
            return false;
        }
        guard.broadcast(GameEvent::BlockChange {
            x,
            y,
            z,
            block_type: 0,
        });
        true
    }

    pub async fn update_position(
        state: &Arc<RwLock<Self>>,
        player_id: &str,
        x: f32,
        y: f32,
        z: f32,
    ) -> bool {
        let mut guard = state.write().await;
        let Some(player) = guard.players.get_mut(player_id) else {
            return false;
        };
        player.x = x;
        player.y = y;
        player.z = z;
        guard.broadcast(GameEvent::PlayerMove {
            player_id: player_id.to_string(),
            x,
            y,
            z,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{GameEvent, GameState};
    use crate::world::World;

    async fn next_event(receiver: &mut tokio::sync::broadcast::Receiver<GameEvent>) -> GameEvent {
        receiver.recv().await.expect("expected broadcast event")
    }

    #[tokio::test]
    async fn join_registers_player_and_broadcasts() {
        let state = GameState::new();
        let mut receiver = state.read().await.subscribe();
        let player = GameState::join(&state, "alice".into()).await;

        assert_eq!(player.name, "alice");
        assert!(state.read().await.players.contains_key(&player.id));

        match next_event(&mut receiver).await {
            GameEvent::PlayerJoin(joined) => {
                assert_eq!(joined.id, player.id);
                assert_eq!(joined.name, "alice");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn join_places_player_near_spawn_position() {
        let state = GameState::new();
        let player = GameState::join(&state, "spawn".into()).await;
        let (x, y, z) = World::spawn_position();
        assert_eq!(player.y, y);
        assert!((player.x - x).abs() <= 3.0);
        assert!((player.z - z).abs() <= 3.0);
    }

    #[tokio::test]
    async fn set_block_updates_world_and_broadcasts() {
        let state = GameState::new();
        let mut receiver = state.read().await.subscribe();

        assert!(GameState::set_block(&state, 1, 2, 3, 5).await);
        assert_eq!(state.read().await.world.get_block(1, 2, 3), Some(5));

        match next_event(&mut receiver).await {
            GameEvent::BlockChange {
                x,
                y,
                z,
                block_type,
            } => {
                assert_eq!((x, y, z, block_type), (1, 2, 3, 5));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn set_block_out_of_bounds_does_not_broadcast() {
        let state = GameState::new();
        let mut receiver = state.read().await.subscribe();

        assert!(!GameState::set_block(&state, -1, 0, 0, 1).await);
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn break_block_clears_voxel_and_broadcasts_air() {
        let state = GameState::new();
        let mut receiver = state.read().await.subscribe();
        GameState::set_block(&state, 4, 5, 6, 2).await;
        let _ = next_event(&mut receiver).await;

        assert!(GameState::break_block(&state, 4, 5, 6).await);
        assert_eq!(state.read().await.world.get_block(4, 5, 6), Some(0));

        match next_event(&mut receiver).await {
            GameEvent::BlockChange {
                x,
                y,
                z,
                block_type,
            } => {
                assert_eq!((x, y, z, block_type), (4, 5, 6, 0));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_position_moves_player_and_broadcasts() {
        let state = GameState::new();
        let mut receiver = state.read().await.subscribe();
        let player = GameState::join(&state, "mover".into()).await;
        let _ = next_event(&mut receiver).await;

        assert!(GameState::update_position(&state, &player.id, 1.5, 2.5, 3.5).await);
        let stored = state.read().await.players.get(&player.id).cloned().unwrap();
        assert!((stored.x, stored.y, stored.z) == (1.5, 2.5, 3.5));

        match next_event(&mut receiver).await {
            GameEvent::PlayerMove { player_id, x, y, z } => {
                assert_eq!(player_id, player.id);
                assert!((x, y, z) == (1.5, 2.5, 3.5));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_position_unknown_player_fails() {
        let state = GameState::new();
        assert!(!GameState::update_position(&state, "missing", 0.0, 0.0, 0.0).await);
    }

    #[tokio::test]
    async fn multiple_subscribers_receive_same_event() {
        let state = GameState::new();
        let mut first = state.read().await.subscribe();
        let mut second = state.read().await.subscribe();

        GameState::set_block(&state, 0, 0, 0, 9).await;

        assert!(matches!(
            next_event(&mut first).await,
            GameEvent::BlockChange {
                x: 0,
                y: 0,
                z: 0,
                block_type: 9
            }
        ));
        assert!(matches!(
            next_event(&mut second).await,
            GameEvent::BlockChange {
                x: 0,
                y: 0,
                z: 0,
                block_type: 9
            }
        ));
    }
}
