"""Entry point for the Grahamcraft Ursina client."""

import grahamcraft2.client.gl_compat  # noqa: F401

from grahamcraft2.client.app import game, main


def input(key: str) -> None:
    """Ursina input hook for block placement and breaking."""
    game.handle_input(key)


if __name__ == "__main__":
    main()
