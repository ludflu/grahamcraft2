"""Ray picking for placing and breaking voxels."""

from __future__ import annotations

from typing import TYPE_CHECKING

from ursina import Entity, Vec3, camera, raycast, scene

if TYPE_CHECKING:
    from grahamcraft2.client.app import Game


def voxel_under_aim(game: Game, distance: float = 12):
    """Return the nearest voxel hit along the camera crosshair."""
    direction = camera.forward
    ignore = game._raycast_ignore()
    best = None
    for origin in _aim_origins(game):
        hit = raycast(
            origin,
            direction,
            distance=distance,
            traverse_target=scene,
            ignore=ignore,
        )
        if not hit.hit or not hasattr(hit.entity, "coord"):
            continue
        if best is None or hit.distance < best.distance:
            best = hit
    return best


def _aim_origins(game: Game) -> tuple[Vec3, ...]:
    """Cast from several points along the view to reduce third-person misses."""
    camera_origin = camera.world_position + camera.forward * 0.05
    eye_origin = game.player.camera_pivot.world_position + camera.forward * 0.05
    return (camera_origin, eye_origin)
