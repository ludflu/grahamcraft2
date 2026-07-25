"""Shared datatypes for the game client."""

from dataclasses import dataclass


@dataclass(frozen=True)
class BlockCoord:
    """Integer voxel position in the world grid."""

    x: int
    y: int
    z: int


@dataclass(frozen=True)
class BlockUpdate:
    """A block placed or removed by the server."""

    coord: BlockCoord
    block_type: int


@dataclass(frozen=True)
class RemotePlayerState:
    """Position and identity of a connected player."""

    player_id: str
    player_name: str
    x: float
    y: float
    z: float


@dataclass(frozen=True)
class PlayerJoin:
    """Another player appeared in the world."""

    state: RemotePlayerState


@dataclass(frozen=True)
class PlayerMove:
    """Another player changed position."""

    player_id: str
    x: float
    y: float
    z: float


@dataclass(frozen=True)
class PlayerLeave:
    """Another player disconnected."""

    player_id: str
