"""Screen-center crosshair for block targeting."""

from __future__ import annotations

from ursina import Entity, camera, color


class Crosshair:
    """A small plus-shaped reticle in the middle of the screen."""

    def __init__(self) -> None:
        """Hide the default controller cursor and draw a thin cross."""
        tint = color.rgba(255, 255, 255, 190)
        Entity(
            parent=camera.ui,
            model="quad",
            color=tint,
            scale=(0.001, 0.00012),
            z=-1,
        )
        Entity(
            parent=camera.ui,
            model="quad",
            color=tint,
            scale=(0.00012, 0.001),
            z=-1,
        )

    @staticmethod
    def hide_controller_cursor(controller) -> None:
        """Turn off the large FirstPersonController diamond cursor."""
        controller.cursor.enabled = False
