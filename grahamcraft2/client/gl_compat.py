"""Disable broken Ursina GLSL shaders on macOS before they are imported."""

from __future__ import annotations

import platform
import sys
import types

_MACOS = platform.system() == "Darwin"


def _install_stub_modules() -> None:
    """Reserve module names and expose None so Ursina skips custom shaders."""
    unlit = types.ModuleType("ursina.shaders.unlit_shader")
    fog = types.ModuleType("ursina.shaders.unlit_with_fog_shader")
    unlit.unlit_shader = None
    fog.unlit_with_fog_shader = None
    sys.modules["ursina.shaders.unlit_shader"] = unlit
    sys.modules["ursina.shaders.unlit_with_fog_shader"] = fog


def patch_loaded_ursina_modules() -> None:
    """Keep macOS on the built-in color pipeline after Ursina imports."""
    if not _MACOS:
        return
    _install_stub_modules()
    entity_module = sys.modules.get("ursina.entity")
    if entity_module is not None:
        entity_module.unlit_shader = None
        entity_module.unlit_with_fog_shader = None
        entity_module.Entity.default_shader = None
    button_module = sys.modules.get("ursina.prefabs.button")
    if button_module is not None:
        button_module.unlit_shader = None


if _MACOS:
    from panda3d.core import loadPrcFileData

    loadPrcFileData("", "gl-version 2 1")
    _install_stub_modules()
