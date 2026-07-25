"""Ursina client for the Grahamcraft multiplayer voxel game."""

from __future__ import annotations

import argparse
import grahamcraft2.client.gl_compat  # noqa: F401

import queue
from typing import Final

from ursina import AmbientLight, DirectionalLight, Entity, Ursina, Vec3, application, camera, color, raycast, scene, window

from grahamcraft2.client.gl_compat import patch_loaded_ursina_modules

from grahamcraft2.client.aiming import voxel_under_aim
from grahamcraft2.client.crosshair import Crosshair
from grahamcraft2.client.arrow_key_controller import ArrowKeyController
from grahamcraft2.client.models import BlockCoord, PlayerJoin, PlayerLeave, PlayerMove
from grahamcraft2.client.player_model import PlayerAvatar
from grahamcraft2.client.remote_players import RemotePlayers
from grahamcraft2.client.rpc import DEFAULT_SERVER, GameSession
from grahamcraft2.client.voxels import VoxelWorld

DEFAULT_PORT: Final = 50051


class Game:
    """Main game state, matching the structure of grahamcraft v1."""

    def __init__(self, server: str = DEFAULT_SERVER) -> None:
        """Create the Ursina app, player, and server session."""
        patch_loaded_ursina_modules()
        Entity.default_shader = None
        self.server = server
        self.app = Ursina(icon="", editor_ui_enabled=False)
        window.color = color.rgb32(135, 206, 235)
        scene.fog_density = (0, 99999)
        AmbientLight(color=color.rgba(1, 1, 1, 1))
        DirectionalLight()
        self.block_events: queue.SimpleQueue = queue.SimpleQueue()
        self.player_events: queue.SimpleQueue = queue.SimpleQueue()
        self.session = GameSession(
            server,
            self.block_events,
            self.player_events,
        )
        self.world = VoxelWorld()
        self.remote_players = RemotePlayers()
        self.player = ArrowKeyController(
            initial_position=Vec3(0, 0, 0),
            gravity=1,
            session=self.session,
            on_before_update=self.drain_network_events,
        )
        Crosshair.hide_controller_cursor(self.player)
        self.crosshair = Crosshair()
        self.avatar = PlayerAvatar(self.player)
        self.remote_players.set_raycast_ignore(self._raycast_ignore)
        GameLoop(self)

    def drain_network_events(self) -> None:
        """Apply block and player updates from the server."""
        self.drain_block_events()
        self.drain_player_events()

    def drain_block_events(self) -> None:
        """Apply every block update the server has sent."""
        while True:
            try:
                update = self.block_events.get_nowait()
            except queue.Empty:
                return
            self.world.apply(update)

    def drain_player_events(self) -> None:
        """Apply every player update the server has sent."""
        while True:
            try:
                event = self.player_events.get_nowait()
            except queue.Empty:
                return
            if isinstance(event, PlayerJoin):
                self.remote_players.apply_join(event)
                if event.state.player_id != self.session.player_id:
                    pos = self.player.position
                    self.session.send_position(pos.x, pos.y, pos.z)
            elif isinstance(event, PlayerMove):
                self.remote_players.apply_move(event)
            else:
                self.remote_players.apply_leave(event)

    def _raycast_ignore(self) -> list[Entity]:
        """Entities that should not block player raycasts."""
        return [
            self.player,
            self.player.camera_pivot,
            self.avatar.figure.root,
            *self.avatar.raycast_ignore,
            *self.remote_players.raycast_ignore(),
        ]

    def _voxel_under_aim(self):
        """Return a raycast hit on a voxel aligned with the camera crosshair."""
        return voxel_under_aim(self)

    def place_from_raycast(self) -> None:
        """Place a block on the face clicked by the player."""
        hit = self._voxel_under_aim()
        if hit is None:
            return
        target = hit.entity.position + hit.normal
        coord = BlockCoord(
            round(target.x),
            round(target.y),
            round(target.z),
        )
        self.session.place_block(coord)

    def break_hovered(self) -> None:
        """Remove the voxel currently under the crosshair."""
        hit = self._voxel_under_aim()
        if hit is None:
            return
        self.session.break_block(hit.entity.coord)

    def handle_input(self, key: str) -> None:
        """Handle mouse clicks for placing and breaking blocks."""
        if key == "escape":
            application.quit()
        elif key == "left mouse down":
            self.place_from_raycast()
        elif key == "right mouse down":
            self.break_hovered()

    def _snap_player_to_ground(self) -> None:
        """Place the player on top of the loaded floor."""
        hit = raycast(
            self.player.world_position + Vec3(0, self.player.height, 0),
            Vec3(0, -1, 0),
            distance=self.player.height + 10,
            traverse_target=scene,
            ignore=self._raycast_ignore(),
        )
        if not hit.hit:
            return
        self.player.y = hit.world_point.y
        self.player.grounded = True

    def run(self) -> None:
        """Connect to the server and start the Ursina main loop."""
        self.session.start()
        if not self.session.wait_until_ready():
            print("Failed to connect to the game server.")
            return
        self.remote_players.set_local_player(self.session.player_id)
        self.drain_block_events()
        x, y, z = self.session.spawn_position
        self.player.position = Vec3(x, y, z)
        self.player.last_position = Vec3(x, y, z)
        self._snap_player_to_ground()
        pos = self.player.position
        self.session.send_position(pos.x, pos.y, pos.z)
        self.app.run()


class GameLoop(Entity):
    """Runs networking sync on Ursina's entity update loop."""

    def __init__(self, game: Game) -> None:
        """Store a reference to the game instance."""
        super().__init__()
        self.game = game

    def update(self) -> None:
        """Apply server updates and animate remote players each frame."""
        self.game.drain_network_events()
        self.game.remote_players.tick()


game: Game | None = None


def normalize_server_address(server: str) -> str:
    """Accept host or host:port and default the gRPC port."""
    if ":" in server:
        return server
    return f"{server}:{DEFAULT_PORT}"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse client command-line options."""
    parser = argparse.ArgumentParser(description="Grahamcraft multiplayer client")
    parser.add_argument(
        "--server",
        "-s",
        default=DEFAULT_SERVER,
        help=f"game server address as host:port (default: {DEFAULT_SERVER})",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    """Launch the Ursina client."""
    global game
    args = parse_args(argv)
    game = Game(normalize_server_address(args.server))
    game.run()


if __name__ == "__main__":
    main()
