"""gRPC client that talks to the game server in a background thread."""

from __future__ import annotations

import queue
import threading
import time
from typing import Final

import grpc

from grahamcraft2.client.models import BlockCoord, BlockUpdate, PlayerJoin, PlayerLeave, PlayerMove, RemotePlayerState
from grahamcraft2.generated import game_pb2, game_pb2_grpc

DEFAULT_SERVER: Final = "localhost:50051"
DEFAULT_BLOCK_TYPE: Final = 1
PLAYER_NAME: Final = "player"
REQUEST_POLL_SECONDS: Final = 0.05


class GameSession:
    """Runs join and event subscription on a worker thread."""

    def __init__(
        self,
        server: str,
        block_events: queue.SimpleQueue[BlockUpdate],
        player_events: queue.SimpleQueue[PlayerJoin | PlayerMove | PlayerLeave],
    ) -> None:
        """Store connection settings and queues for streamed updates."""
        self._server = server
        self._block_events = block_events
        self._player_events = player_events
        self._player_id = ""
        self._requests: queue.SimpleQueue[tuple[str, BlockCoord]] = queue.SimpleQueue()
        self._latest_position: tuple[float, float, float] | None = None
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._ready = threading.Event()
        self._spawn_position = (0.0, 0.0, 0.0)
        self._connection_error = ""

    @property
    def spawn_position(self) -> tuple[float, float, float]:
        """Return the spawn point assigned by the server on join."""
        return self._spawn_position

    @property
    def player_id(self) -> str:
        """Return the id assigned by the server after join."""
        return self._player_id

    def start(self) -> None:
        """Connect to the server and begin streaming events."""
        self._thread.start()

    def wait_until_ready(self, timeout: float = 10.0) -> bool:
        """Block until the initial world snapshot has been received."""
        if not self._ready.wait(timeout):
            if not self._connection_error:
                self._connection_error = (
                    f"Timed out after {timeout:.0f}s waiting for {self._server}"
                )
            return False
        return not self._connection_error

    @property
    def connection_error(self) -> str:
        """Return a connection failure message, if any."""
        return self._connection_error

    def place_block(self, coord: BlockCoord) -> None:
        """Ask the server to place a block."""
        self._requests.put(("place", coord))

    def break_block(self, coord: BlockCoord) -> None:
        """Ask the server to remove a block."""
        self._requests.put(("break", coord))

    def send_position(self, x: float, y: float, z: float) -> None:
        """Queue the latest player position for the server."""
        self._latest_position = (x, y, z)

    def _run(self) -> None:
        """Maintain the gRPC connection until the process exits."""
        try:
            with grpc.insecure_channel(self._server) as channel:
                grpc.channel_ready_future(channel).result(timeout=10)
                stub = game_pb2_grpc.GameServiceStub(channel)
                self._join(stub)
                events = stub.SubscribeEvents(
                    game_pb2.SubscribeRequest(player_id=self._player_id)
                )
                threading.Thread(
                    target=self._read_events,
                    args=(events,),
                    daemon=True,
                ).start()
                while True:
                    self._flush_requests(stub)
                    self._flush_positions(stub)
                    time.sleep(REQUEST_POLL_SECONDS)
        except grpc.FutureTimeoutError:
            self._connection_error = (
                f"Could not reach game server at {self._server}. "
                "Check that the server is running and the IP is correct."
            )
        except grpc.RpcError as exc:
            self._connection_error = (
                f"gRPC error from {self._server}: {exc.code()} {exc.details()}"
            )
        except Exception as exc:
            self._connection_error = (
                f"Failed to connect to {self._server}: {exc}"
            )

    def _read_events(self, events: grpc.CallIterator[game_pb2.GameEvent]) -> None:
        """Forward streamed game events to the main thread."""
        for event in events:
            kind = event.WhichOneof("event")
            if kind == "block_change":
                change = event.block_change
                self._enqueue_block(change.x, change.y, change.z, change.block_type)
            elif kind == "player_move":
                move = event.player_move
                self._enqueue_move(move.player_id, move.x, move.y, move.z)
            elif kind == "player_join":
                joined = event.player_join.player
                if joined is not None:
                    self._enqueue_join(joined)
            elif kind == "player_leave":
                self._enqueue_leave(event.player_leave.player_id)

    def _join(self, stub: game_pb2_grpc.GameServiceStub) -> None:
        """Join the world and enqueue the initial block snapshot."""
        response = stub.Join(game_pb2.JoinRequest(player_name=PLAYER_NAME))
        self._player_id = response.player_id
        for player in response.players:
            if player.player_id == self._player_id:
                self._spawn_position = (player.x, player.y, player.z)
            else:
                self._enqueue_join(player)
        world = response.world
        if world is None:
            self._ready.set()
            return
        for block in world.blocks:
            self._enqueue_block(block.x, block.y, block.z, block.block_type)
        self._ready.set()


    def _flush_requests(self, stub: game_pb2_grpc.GameServiceStub) -> None:
        """Send queued block placement and break requests."""
        while True:
            try:
                action, coord = self._requests.get_nowait()
            except queue.Empty:
                return
            self._send_block_action(stub, action, coord)

    def _flush_positions(self, stub: game_pb2_grpc.GameServiceStub) -> None:
        """Send the most recent player position to the server."""
        if self._latest_position is None:
            return
        x, y, z = self._latest_position
        self._latest_position = None
        stub.UpdatePosition(
            game_pb2.UpdatePositionRequest(
                player_id=self._player_id, x=x, y=y, z=z
            )
        )

    def _send_block_action(
        self,
        stub: game_pb2_grpc.GameServiceStub,
        action: str,
        coord: BlockCoord,
    ) -> None:
        """Call SetBlock or BreakBlock on the server."""
        if action == "place":
            stub.SetBlock(
                game_pb2.SetBlockRequest(
                    player_id=self._player_id,
                    x=coord.x,
                    y=coord.y,
                    z=coord.z,
                    block_type=DEFAULT_BLOCK_TYPE,
                )
            )
            return
        stub.BreakBlock(
            game_pb2.BreakBlockRequest(
                player_id=self._player_id,
                x=coord.x,
                y=coord.y,
                z=coord.z,
            )
        )


    def _enqueue_block(self, x: int, y: int, z: int, block_type: int) -> None:
        """Push a block update onto the main-thread queue."""
        update = BlockUpdate(BlockCoord(x, y, z), block_type)
        self._block_events.put(update)

    def _enqueue_join(self, player: game_pb2.PlayerState) -> None:
        """Push a player join onto the main-thread queue."""
        state = RemotePlayerState(
            player_id=player.player_id,
            player_name=player.player_name,
            x=player.x,
            y=player.y,
            z=player.z,
        )
        self._player_events.put(PlayerJoin(state))

    def _enqueue_move(self, player_id: str, x: float, y: float, z: float) -> None:
        """Push a player move onto the main-thread queue."""
        self._player_events.put(PlayerMove(player_id, x, y, z))

    def _enqueue_leave(self, player_id: str) -> None:
        """Push a player leave onto the main-thread queue."""
        self._player_events.put(PlayerLeave(player_id))
