"""Player controller using arrow keys for movement, based on grahamcraft v1."""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable, Final

from ursina import Vec3, held_keys, raycast, time
from ursina.prefabs.first_person_controller import FirstPersonController

if TYPE_CHECKING:
    from grahamcraft2.client.rpc import GameSession

JUMP_SPEED: Final = 6.5
GRAVITY: Final = 24.0
MAX_FALL_SPEED: Final = 14.0
MAX_FALL_STEP: Final = 0.35


class ArrowKeyController(FirstPersonController):
    """First-person controller with arrow-key movement and position sync."""

    def __init__(
        self,
        initial_position: Vec3,
        session: GameSession | None = None,
        on_before_update: Callable[[], None] | None = None,
        **kwargs: object,
    ) -> None:
        """Place the player and optionally attach a server session."""
        super().__init__(**kwargs)
        self.gravity = 0
        self.session = session
        self.on_before_update = on_before_update
        self.position = initial_position
        self.last_position = initial_position
        self._was_airborne = False
        self._vertical_velocity = 0.0

    def jump(self) -> None:
        """Jump with velocity-based physics instead of Ursina's animate_y."""
        if not self.grounded:
            return
        self.grounded = False
        self._vertical_velocity = JUMP_SPEED
        self.air_time = 0
        if hasattr(self, "y_animator"):
            self.y_animator.pause()

    def update(self) -> None:
        """Move with arrow keys, then run camera and collision updates."""
        if self.on_before_update is not None:
            self.on_before_update()
        speed = self.speed * time.dt

        if held_keys["up arrow"]:
            self.position += self.forward * speed
        if held_keys["down arrow"]:
            self.position -= self.forward * speed
        if held_keys["left arrow"]:
            self.position -= self.right * speed
        if held_keys["right arrow"]:
            self.position += self.right * speed

        super().update()
        self._apply_vertical_physics()
        self._sync_position_to_server()

    def _apply_vertical_physics(self) -> None:
        """Apply capped gravity and snap the player onto voxel tops."""
        if not self.grounded:
            self._vertical_velocity -= GRAVITY * time.dt
            self._vertical_velocity = max(self._vertical_velocity, -MAX_FALL_SPEED)
            step = self._vertical_velocity * time.dt
            if step < 0:
                step = max(step, -MAX_FALL_STEP)
            self.y += step

        ground = self._ground_hit()
        if not ground.hit:
            self.grounded = False
            return

        ground_y = ground.world_point.y
        if self.y < ground_y:
            self.y = ground_y
            self._vertical_velocity = 0
            self.grounded = True
            self.air_time = 0
            return

        on_ground = ground.distance <= self.height + 0.1 and self._vertical_velocity <= 0
        if on_ground:
            self.y = ground_y
            self._vertical_velocity = 0
            self.grounded = True
            self.air_time = 0
            return

        self.grounded = False

    def _ground_hit(self):
        """Return a downward raycast hit against walkable voxels."""
        return raycast(
            self.world_position + Vec3(0, self.height, 0),
            Vec3(0, -1, 0),
            distance=self.height + 8,
            traverse_target=self.traverse_target,
            ignore=self.ignore_list,
        )

    def _sync_position_to_server(self) -> None:
        """Send the latest player position to the server."""
        if not self.session:
            return

        airborne = not self.grounded
        landed = self.grounded and self._was_airborne
        self._was_airborne = airborne

        if not (landed or airborne or self._position_changed()):
            return
        pos = self.position
        self.session.send_position(pos.x, pos.y, pos.z)
        self.last_position = Vec3(pos.x, pos.y, pos.z)

    def _position_changed(self) -> bool:
        """Return True when the player moved enough to notify the server."""
        pos = self.position
        last = self.last_position
        return (
            abs(pos.x - last.x) > 0.01
            or abs(pos.y - last.y) > 0.01
            or abs(pos.z - last.z) > 0.01
        )
