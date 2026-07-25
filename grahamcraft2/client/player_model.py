"""Low-poly blocky avatars for local and remote players."""

from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
from typing import Final

from ursina import Entity, Vec3, camera, color, destroy

from grahamcraft2.client.arrow_key_controller import ArrowKeyController
from grahamcraft2.client.voxels import BLOCK_PALETTE

CAMERA_BACK_OFFSET: Final = Vec3(0, 0.25, -6)


@dataclass(frozen=True, slots=True)
class AvatarColors:
    """Tint colors for a blocky character."""

    skin: color
    body: color
    limbs: color


@dataclass(frozen=True, slots=True)
class _BodyPart:
    """One cube in the blocky character rig."""

    position: Vec3
    scale: Vec3
    slot: str


_BODY_PARTS: Final = (
    _BodyPart(Vec3(0, 1.55, 0), Vec3(0.45, 0.45, 0.45), "skin"),
    _BodyPart(Vec3(0, 1.0, 0), Vec3(0.5, 0.65, 0.3), "body"),
    _BodyPart(Vec3(-0.15, 0.35, 0), Vec3(0.22, 0.7, 0.22), "limbs"),
    _BodyPart(Vec3(0.15, 0.35, 0), Vec3(0.22, 0.7, 0.22), "limbs"),
    _BodyPart(Vec3(-0.38, 1.0, 0), Vec3(0.2, 0.6, 0.2), "body"),
    _BodyPart(Vec3(0.38, 1.0, 0), Vec3(0.2, 0.6, 0.2), "body"),
)

LOCAL_AVATAR_COLORS: Final = AvatarColors(
    skin=color.rgb32(255, 213, 170),
    body=color.azure,
    limbs=color.blue,
)


def colors_for_player(player_id: str) -> AvatarColors:
    """Pick a stable palette color set from a player id."""
    digest = sha256(player_id.encode()).digest()
    body = BLOCK_PALETTE[digest[0] % len(BLOCK_PALETTE)]
    limbs = BLOCK_PALETTE[digest[1] % len(BLOCK_PALETTE)]
    skin = color.rgb32(255, 213, 170)
    return AvatarColors(skin=skin, body=body, limbs=limbs)


def _tint_for_slot(colors: AvatarColors, slot: str) -> color:
    if slot == "skin":
        return colors.skin
    if slot == "limbs":
        return colors.limbs
    return colors.body


class BlockyFigure:
    """A cube-built character rig in the world."""

    def __init__(
        self,
        position: Vec3,
        colors: AvatarColors,
        parent: Entity | None = None,
    ) -> None:
        """Create a blocky avatar at the given position."""
        self.root = Entity(parent=parent, position=position)
        self.parts: list[Entity] = []
        for part in _BODY_PARTS:
            cube = Entity(
                parent=self.root,
                model="cube",
                position=part.position,
                scale=part.scale,
                texture="white_cube",
                color=_tint_for_slot(colors, part.slot),
                collider=None,
                shader=None,
            )
            self.parts.append(cube)

    def set_position(self, x: float, y: float, z: float) -> None:
        """Move the avatar root to a server position."""
        self.root.position = Vec3(x, y, z)

    def remove(self) -> None:
        """Destroy the avatar from the scene."""
        destroy(self.root)
        self.parts.clear()


class PlayerAvatar:
    """Visible blocky character following the local player."""

    def __init__(self, controller: ArrowKeyController) -> None:
        """Build a cube rig and switch to a third-person camera."""
        self.controller = controller
        self.figure = BlockyFigure(
            position=Vec3(0, 0, 0),
            colors=LOCAL_AVATAR_COLORS,
            parent=controller,
        )
        controller.ignore_list.extend(self.figure.parts)
        controller.ignore_list.append(self.figure.root)
        camera.position = CAMERA_BACK_OFFSET

    @property
    def raycast_ignore(self) -> list[Entity]:
        """Entities to exclude from placement and ground raycasts."""
        return self.figure.parts
