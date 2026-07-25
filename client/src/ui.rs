//! Crosshair UI — positioned on the block placement point in the world (see aiming.rs).

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, Window};

#[derive(Resource, Default)]
pub struct CrosshairSmooth {
    pub position: Option<Vec2>,
}

#[derive(Component)]
pub(crate) struct CrosshairBar {
    pub(crate) half_width: f32,
    pub(crate) half_height: f32,
}

pub fn setup_crosshair(mut commands: Commands) {
    let tint = Color::srgba(1.0, 1.0, 1.0, 0.75);
    commands.spawn((
        CrosshairBar {
            half_width: 7.0,
            half_height: 1.0,
        },
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(14.0),
            height: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(tint),
        Visibility::Hidden,
    ));
    commands.spawn((
        CrosshairBar {
            half_width: 1.0,
            half_height: 7.0,
        },
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(2.0),
            height: Val::Px(14.0),
            ..default()
        },
        BackgroundColor(tint),
        Visibility::Hidden,
    ));
}

/// Screen position for the aim ray (window center when the cursor is locked).
pub fn aim_screen_position(window: &Window) -> Vec2 {
    let center = Vec2::new(window.width() * 0.5, window.height() * 0.5);

    let cursor_free = window.cursor_options.visible
        && matches!(window.cursor_options.grab_mode, CursorGrabMode::None);

    if cursor_free {
        window.cursor_position().unwrap_or(center)
    } else {
        center
    }
}
