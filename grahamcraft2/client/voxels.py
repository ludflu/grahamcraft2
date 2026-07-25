"""Ursina voxel entities driven by server block updates."""

from __future__ import annotations

import grahamcraft2.client.gl_compat  # noqa: F401

from typing import Final

from ursina import Entity, color, destroy, scene

from grahamcraft2.client.models import BlockCoord, BlockUpdate

BLOCK_COLOR_NAMES: Final = (
    "red",
    "orange",
    "yellow",
    "lime",
    "green",
    "turquoise",
    "azure",
    "blue",
    "violet",
    "magenta",
)
BLOCK_PALETTE: Final = tuple(color.colors[name] for name in BLOCK_COLOR_NAMES)


def block_color(block_type: int) -> color:
    """Map a server block type to one of ten Ursina palette colors."""
    return BLOCK_PALETTE[(block_type - 1) % len(BLOCK_PALETTE)]


class Voxel(Entity):
    """A collidable cube rendered at a server block coordinate."""

    def __init__(self, coord: BlockCoord, block_type: int) -> None:
        """Create a cube for the given server block."""
        self.coord = coord
        tint = block_color(block_type)
        super().__init__(
            parent=scene,
            position=(coord.x, coord.y, coord.z),
            model="cube",
            origin_y=0.5,
            texture="white_cube",
            color=tint,
            collider="mesh",
            shader=None,
        )
        self.color = tint


class VoxelWorld:
    """Tracks rendered voxels and applies server block updates."""

    def __init__(self) -> None:
        """Start with an empty client-side block map."""
        self._voxels: dict[BlockCoord, Voxel] = {}

    def apply(self, update: BlockUpdate) -> None:
        """Add, replace, or remove a voxel from a server update."""
        if update.block_type == 0:
            self._remove(update.coord)
            return
        self._upsert(update.coord, update.block_type)

    def _remove(self, coord: BlockCoord) -> None:
        """Destroy a rendered voxel when the server clears the cell."""
        voxel = self._voxels.pop(coord, None)
        if voxel is not None:
            destroy(voxel)

    def _upsert(self, coord: BlockCoord, block_type: int) -> None:
        """Create or refresh a voxel for a non-air block."""
        existing = self._voxels.get(coord)
        if existing is not None:
            destroy(existing)
        self._voxels[coord] = Voxel(coord, block_type)
