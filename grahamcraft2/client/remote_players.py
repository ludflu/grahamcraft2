"""Render other connected players from server events."""

from __future__ import annotations

from collections.abc import Callable
from typing import Final

from ursina import Entity, Vec3, lerp, raycast, scene, time

from grahamcraft2.client.models import PlayerJoin, PlayerLeave, PlayerMove, RemotePlayerState
from grahamcraft2.client.player_model import BlockyFigure, colors_for_player

FLOOR_TOP_Y: Final = 1.0
LERP_SPEED: Final = 14.0


def _ground_y_at(x: float, z: float, ignore: list[Entity]) -> float:
    """Raycast down to the voxel floor and return feet height."""
    hit = raycast(
        Vec3(x, 10, z),
        Vec3(0, -1, 0),
        distance=20,
        traverse_target=scene,
        ignore=ignore,
    )
    if hit.hit:
        return hit.world_point.y
    return FLOOR_TOP_Y


class RemotePlayers:
    """Track and display avatars for every other connected player."""

    def __init__(self) -> None:
        """Start with no remote avatars."""
        self._local_player_id = ""
        self._figures: dict[str, BlockyFigure] = {}
        self._targets: dict[str, Vec3] = {}
        self._raycast_ignore: Callable[[], list[Entity]] = list

    def set_local_player(self, player_id: str) -> None:
        """Ignore updates for the local player id."""
        self._local_player_id = player_id

    def set_raycast_ignore(self, supplier: Callable[[], list[Entity]]) -> None:
        """Provide entities to skip when snapping avatars to the ground."""
        self._raycast_ignore = supplier

    def apply_join(self, event: PlayerJoin) -> None:
        """Spawn an avatar when another player connects."""
        state = event.state
        if state.player_id == self._local_player_id:
            return
        self._spawn(state, snap_to_ground=True)

    def apply_move(self, event: PlayerMove) -> None:
        """Track a remote avatar position from the server."""
        if event.player_id == self._local_player_id:
            return
        target = Vec3(event.x, event.y, event.z)
        self._targets[event.player_id] = target
        if event.player_id not in self._figures:
            state = RemotePlayerState(
                player_id=event.player_id,
                player_name="player",
                x=event.x,
                y=event.y,
                z=event.z,
            )
            self._spawn(state, snap_to_ground=False)

    def apply_leave(self, event: PlayerLeave) -> None:
        """Remove a remote avatar when a player disconnects."""
        self._targets.pop(event.player_id, None)
        figure = self._figures.pop(event.player_id, None)
        if figure is not None:
            figure.remove()

    def tick(self) -> None:
        """Smoothly move remote avatars toward their latest server positions."""
        step = min(time.dt * LERP_SPEED, 1.0)
        for player_id, figure in self._figures.items():
            target = self._targets.get(player_id)
            if target is None:
                continue
            current = figure.root.position
            figure.root.position = Vec3(
                lerp(current.x, target.x, step),
                lerp(current.y, target.y, step),
                lerp(current.z, target.z, step),
            )

    def raycast_ignore(self) -> list[Entity]:
        """All remote avatar cubes to skip in raycasts."""
        ignored: list[Entity] = []
        for figure in self._figures.values():
            ignored.extend(figure.parts)
        return ignored

    def _spawn(self, state: RemotePlayerState, *, snap_to_ground: bool) -> None:
        """Create or replace a remote avatar."""
        existing = self._figures.pop(state.player_id, None)
        if existing is not None:
            existing.remove()
        feet_y = (
            _ground_y_at(state.x, state.z, self._raycast_ignore())
            if snap_to_ground
            else state.y
        )
        target = Vec3(state.x, feet_y, state.z)
        self._targets[state.player_id] = target
        figure = BlockyFigure(
            position=target,
            colors=colors_for_player(state.player_id),
            parent=scene,
        )
        self._figures[state.player_id] = figure
