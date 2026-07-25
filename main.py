"""Entry point for the Grahamcraft Ursina client."""

import grahamcraft2.client.gl_compat  # noqa: F401

from grahamcraft2.client import app


def input(key: str) -> None:
    """Ursina input hook for block placement and breaking."""
    if app.game is not None:
        app.game.handle_input(key)


if __name__ == "__main__":
    app.main()
